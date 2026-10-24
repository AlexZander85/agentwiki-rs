use super::{
    bounded_redacted_output, clean_path, dedup_paths, ok_or_flag, parse_ts, redacted_truncate,
    title_from_messages, Adapter, Discovered,
};
use crate::model::{EditEvent, EditKind, Message, Role, Session, ToolCall, ToolResult};
use crate::util::short_id;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub struct Antigravity;

const SNIPPET_CAP: usize = 200;

fn edited_path(name: &str, input: Option<&Value>) -> Option<String> {
    let writes = matches!(name, "write_file" | "edit_file");
    if !writes {
        return None;
    }
    let input = input?;
    for key in ["Path", "path", "file_path"] {
        if let Some(p) = input.get(key).and_then(Value::as_str) {
            if !p.is_empty() {
                return Some(p.to_string());
            }
        }
    }
    None
}

fn edit_event(
    name: &str,
    input: Option<&Value>,
    ts: Option<chrono::DateTime<chrono::Utc>>,
) -> Option<EditEvent> {
    let path = clean_path(&edited_path(name, input)?)?;
    let input = input?;
    let kind = match name {
        "edit_file" => EditKind::Edit,
        "write_file" => EditKind::Write,
        _ => return None,
    };
    let raw = match kind {
        EditKind::Edit => input
            .get("Replacement")
            .or_else(|| input.get("replacement"))
            .and_then(Value::as_str)
            .unwrap_or(""),
        EditKind::Write => input
            .get("Content")
            .or_else(|| input.get("content"))
            .and_then(Value::as_str)
            .unwrap_or(""),
        _ => "",
    };
    let snippet = redacted_truncate(raw.trim(), SNIPPET_CAP);
    Some(EditEvent {
        path,
        kind,
        snippet,
        ts,
    })
}

fn redact_json_strings(value: &mut Value) {
    match value {
        Value::String(text) => *text = crate::redact::redact(text).into_owned(),
        Value::Array(values) => values.iter_mut().for_each(redact_json_strings),
        Value::Object(values) => values.values_mut().for_each(redact_json_strings),
        _ => {}
    }
}

fn extract_user_request(text: &str) -> String {
    if let Some(start) = text.find("<USER_REQUEST>") {
        let after = &text[start + "<USER_REQUEST>".len()..];
        if let Some(end) = after.find("</USER_REQUEST>") {
            return after[..end].trim().to_string();
        }
    }
    text.trim().to_string()
}

fn conversation_id(path: &Path) -> Option<String> {
    let mut current = path.parent()?;
    if current.file_name().is_some_and(|n| n == "logs") {
        current = current.parent()?;
    }
    if current
        .file_name()
        .is_some_and(|n| n == ".system_generated")
    {
        current = current.parent()?;
    }
    current
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
}

impl Adapter for Antigravity {
    fn name(&self) -> &'static str {
        "antigravity"
    }

    fn root(&self) -> Option<PathBuf> {
        Some(
            dirs::home_dir()?
                .join(".gemini")
                .join("antigravity")
                .join("brain"),
        )
    }

    fn reconcile_scope(&self) -> Option<String> {
        super::root_scope(self.root().as_deref())
    }

    fn discover(&self) -> Discovered {
        let Some(root) = self.root() else {
            return Vec::new().into();
        };
        if !root.exists() {
            return Vec::new().into();
        }
        let mut had_error = false;
        let files: Vec<PathBuf> = WalkDir::new(root)
            .min_depth(2)
            .max_depth(5)
            .into_iter()
            .filter_map(|e| ok_or_flag(e, &mut had_error))
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.file_name() == "transcript.jsonl" || e.file_name() == "transcript_full.jsonl"
            })
            .map(|e| e.into_path())
            .collect();

        // If both transcript.jsonl and transcript_full.jsonl exist in the same folder, keep transcript.jsonl
        let mut dedupped: Vec<PathBuf> = Vec::new();
        for f in files {
            if f.file_name().is_some_and(|n| n == "transcript_full.jsonl") {
                if let Some(p) = f.parent() {
                    if p.join("transcript.jsonl").exists() {
                        continue;
                    }
                }
            }
            dedupped.push(f);
        }

        Discovered {
            files: dedupped,
            had_error,
        }
    }

    fn parse(&self, path: &Path) -> Result<Session> {
        let (lines, _windowed) = crate::util::session_lines_keeping(path, |line| {
            line.contains("\"type\":\"USER_INPUT\"")
                || line.contains("\"type\":\"PLANNER_RESPONSE\"")
        })?;

        let mut messages: Vec<Message> = Vec::new();
        let mut touched: Vec<String> = Vec::new();
        let mut edits: Vec<EditEvent> = Vec::new();
        let mut project: Option<String> = None;
        let mut started = None;
        let mut ended = None;

        for line in &lines {
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };

            let ts = v
                .get("created_at")
                .and_then(Value::as_str)
                .and_then(parse_ts);
            if let Some(t) = ts {
                if started.is_none() {
                    started = Some(t);
                }
                ended = Some(t);
            }

            let step_type = v.get("type").and_then(Value::as_str).unwrap_or("");
            match step_type {
                "USER_INPUT" => {
                    if let Some(c) = v.get("content").and_then(Value::as_str) {
                        let clean = extract_user_request(c);
                        if !clean.is_empty() {
                            messages.push(Message {
                                role: Role::User,
                                text: clean,
                                ts,
                                tool: None,
                            });
                        }
                    }
                }
                "PLANNER_RESPONSE" => {
                    if let Some(c) = v.get("content").and_then(Value::as_str) {
                        let trimmed = c.trim();
                        if !trimmed.is_empty() {
                            messages.push(Message {
                                role: Role::Assistant,
                                text: trimmed.to_string(),
                                ts,
                                tool: None,
                            });
                        }
                    }

                    if let Some(Value::Array(tcs)) = v.get("tool_calls") {
                        for tc in tcs {
                            let name = tc.get("name").and_then(Value::as_str).unwrap_or("?");
                            let input = tc.get("args").or_else(|| tc.get("arguments"));
                            if project.is_none() {
                                if let Some(inp) = input {
                                    if let Some(cwd) = inp
                                        .get("Cwd")
                                        .or_else(|| inp.get("cwd"))
                                        .and_then(Value::as_str)
                                    {
                                        project = Some(cwd.to_string());
                                    } else if let Some(p) = inp
                                        .get("Path")
                                        .or_else(|| inp.get("path"))
                                        .and_then(Value::as_str)
                                    {
                                        if let Some(parent) = Path::new(p).parent() {
                                            project = Some(parent.to_string_lossy().into_owned());
                                        }
                                    }
                                }
                            }

                            if let Some(ev) = edit_event(name, input, ts) {
                                touched.push(ev.path.clone());
                                edits.push(ev);
                            }

                            let mut args = input.cloned().unwrap_or_else(|| json!({}));
                            redact_json_strings(&mut args);
                            messages.push(Message::tool_call(
                                ts,
                                ToolCall {
                                    id: None,
                                    name: name.to_string(),
                                    args,
                                },
                            ));
                        }
                    }
                }
                "GENERIC" => {
                    if let Some(c) = v.get("content").and_then(Value::as_str) {
                        let output = bounded_redacted_output(c);
                        messages.push(Message::tool_result(
                            ts,
                            ToolResult {
                                call_id: None,
                                text: output,
                                is_error: v.get("status").and_then(Value::as_str) == Some("ERROR"),
                            },
                        ));
                    }
                }
                _ => {}
            }
        }

        let cid = conversation_id(path).unwrap_or_else(|| short_id(&path.to_string_lossy()));
        let title = title_from_messages(&messages);

        Ok(Session {
            id: cid,
            tool: "antigravity",
            path: path.to_path_buf(),
            project: project.unwrap_or_else(|| "antigravity".to_string()),
            started,
            ended,
            title,
            subagent: false,
            parent_id: None,
            messages,
            touched: dedup_paths(touched),
            edits,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_antigravity_edit_event() {
        let input = json!({
            "Path": "d:/project/main.rs",
            "Replacement": "fn main() { println!(\"hello\"); }",
            "Target": "fn main() {}"
        });
        let ev =
            edit_event("edit_file", Some(&input), None).expect("edit_event should parse edit_file");
        assert_eq!(ev.path, "d:/project/main.rs");
        assert!(ev.snippet.contains("println!"));

        let write_input = json!({
            "Path": "d:/project/new.rs",
            "Content": "let a = 123;"
        });
        let wev = edit_event("write_file", Some(&write_input), None)
            .expect("edit_event should parse write_file");
        assert_eq!(wev.path, "d:/project/new.rs");
        assert!(wev.snippet.contains("let a = 123;"));
    }
}
