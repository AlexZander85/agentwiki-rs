//! Backup and sync wikiagents data to a private GitHub repository.

use anyhow::{Context, Result};
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const MAX_CHUNK_BYTES: u64 = 45 * 1024 * 1024; // 45 MB chunks (safely under GitHub's 100 MB hard limit)
pub const REPO_URL_KEY: &str = "github_backup_repo";
pub const LAST_BACKUP_KEY: &str = "github_backup_last_at";
pub const LAST_COMMIT_KEY: &str = "github_backup_last_commit";
pub const LAST_STATUS_KEY: &str = "github_backup_last_status";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub repo_url: String,
    pub last_backup_at: Option<i64>,
    pub last_commit: Option<String>,
    pub last_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupReport {
    pub success: bool,
    pub commit_hash: Option<String>,
    pub total_sessions: i64,
    pub total_files_backed_up: usize,
    pub message: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTestResult {
    pub ok: bool,
    pub message: String,
    pub git_version: String,
    pub git_user: Option<String>,
}

/// Retrieve backup configuration from the index meta table.
pub fn get_backup_config(conn: &Connection) -> Result<BackupConfig> {
    let repo_url = get_meta(conn, REPO_URL_KEY)?.unwrap_or_default();
    let last_backup_at = get_meta(conn, LAST_BACKUP_KEY)?
        .and_then(|s| s.parse::<i64>().ok());
    let last_commit = get_meta(conn, LAST_COMMIT_KEY)?;
    let last_status = get_meta(conn, LAST_STATUS_KEY)?;

    Ok(BackupConfig {
        repo_url,
        last_backup_at,
        last_commit,
        last_status,
    })
}

/// Save GitHub repository URL into the index meta table.
pub fn set_backup_config(conn: &Connection, repo_url: &str) -> Result<()> {
    set_meta(conn, REPO_URL_KEY, repo_url.trim())
}

/// Test connection to the remote repository using `git ls-remote`.
pub fn test_connection(repo_url: &str) -> ConnectionTestResult {
    let git_version = Command::new("git")
        .arg("--version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "git not found".to_string());

    let git_user = Command::new("git")
        .args(["config", "--get", "user.name"])
        .output()
        .ok()
        .and_then(|o| {
            let u = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if u.is_empty() {
                None
            } else {
                Some(u)
            }
        });

    if repo_url.trim().is_empty() {
        return ConnectionTestResult {
            ok: false,
            message: "URL репозитория не указан".to_string(),
            git_version,
            git_user,
        };
    }

    match Command::new("git")
        .args(["ls-remote", repo_url.trim()])
        .output()
    {
        Ok(output) if output.status.success() => ConnectionTestResult {
            ok: true,
            message: "Подключение успешно! Доступ к приватному репозиторию подтвержден.".to_string(),
            git_version,
            git_user,
        },
        Ok(output) => {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            let msg = if err.is_empty() {
                format!("Ошибка проверки доступа (код {})", output.status)
            } else {
                err
            };
            ConnectionTestResult {
                ok: false,
                message: msg,
                git_version,
                git_user,
            }
        }
        Err(e) => ConnectionTestResult {
            ok: false,
            message: format!("Не удалось вызвать git: {e}"),
            git_version,
            git_user,
        },
    }
}

/// Resolve the local staging directory where the backup Git repository lives.
pub fn resolve_backup_dir() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("WIKIAGENTS_BACKUP_DIR") {
        let p = PathBuf::from(dir);
        fs::create_dir_all(&p)?;
        return Ok(p);
    }

    let base = match crate::index::db_path() {
        Ok(p) => p.parent().unwrap_or(Path::new(".")).to_path_buf(),
        Err(_) => PathBuf::from("data"),
    };
    let backup_dir = base.join("backup_repo");
    fs::create_dir_all(&backup_dir)?;
    Ok(backup_dir)
}

/// Execute full backup to GitHub:
/// 1. Exports curated user data (favorites, tags, notes, snippets, summaries).
/// 2. Exports raw web session JSONs (chunking files > 45 MB).
/// 3. Exports full SQLite database (vacuum -> gzip -> 45 MB chunks).
/// 4. Generates manifest.json and README.md.
/// 5. Commits and pushes to GitHub.
pub fn run_backup(conn: &Connection, repo_url: &str) -> Result<BackupReport> {
    let repo_url = repo_url.trim();
    if repo_url.is_empty() {
        anyhow::bail!("GitHub repository URL is empty");
    }

    let backup_dir = resolve_backup_dir()?;
    let now = Utc::now();
    let timestamp = now.timestamp();

    // 1. Initialize or configure git repo in backup_dir
    ensure_git_repo(&backup_dir, repo_url)?;

    // 2. Export curated tables
    let curated_dir = backup_dir.join("curated");
    fs::create_dir_all(&curated_dir)?;
    export_curated_tables(conn, &curated_dir)?;

    // 3. Export web session files (with 45MB chunking)
    let web_dir = backup_dir.join("web_session");
    fs::create_dir_all(&web_dir)?;
    export_web_session_files(&web_dir)?;

    // 4. Export database snapshot (vacuum -> gzip -> 45MB chunking)
    let db_dir = backup_dir.join("database");
    fs::create_dir_all(&db_dir)?;
    export_database_snapshot(conn, &db_dir)?;

    // 5. Generate manifest and README
    let (total_sessions, tool_stats) = query_session_stats(conn)?;
    write_manifest_and_readme(&backup_dir, repo_url, total_sessions, &tool_stats)?;

    // 6. Git commit and push
    run_git_cmd(&backup_dir, &["add", "-A"])?;

    let status = run_git_cmd(&backup_dir, &["status", "--porcelain"])?;
    let commit_hash;
    let message;

    if status.is_empty() {
        commit_hash = run_git_cmd(&backup_dir, &["rev-parse", "--short", "HEAD"]).ok();
        let _ = run_git_cmd(&backup_dir, &["push", "-u", "origin", "main"]);
        message = "Данные уже синхронизированы с GitHub, изменений нет.".to_string();
    } else {
        let commit_msg = format!(
            "wikiagents backup: {} ({} sessions across {} tools)",
            now.format("%Y-%m-%d %H:%M:%S UTC"),
            total_sessions,
            tool_stats.len()
        );
        run_git_cmd(&backup_dir, &["commit", "-m", &commit_msg])?;
        commit_hash = Some(run_git_cmd(&backup_dir, &["rev-parse", "--short", "HEAD"])?);

        // Push to origin main with retries and force fallback if history diverged
        let mut push_res = run_git_cmd(&backup_dir, &["push", "-u", "origin", "main"]);
        if push_res.is_err() {
            push_res = run_git_cmd(&backup_dir, &["push", "-u", "origin", "main", "--force"]);
            if push_res.is_err() {
                for attempt in 1..=3 {
                    std::thread::sleep(std::time::Duration::from_secs(2 * attempt));
                    push_res = run_git_cmd(&backup_dir, &["push", "-u", "origin", "main", "--force"]);
                    if push_res.is_ok() {
                        break;
                    }
                }
            }
        }
        push_res?;
        message = format!(
            "Успешно выгружено в GitHub! Коммит: {}, всего сессий: {}",
            commit_hash.as_deref().unwrap_or(""),
            total_sessions
        );
    }

    // 7. Save metadata in index
    set_meta(conn, REPO_URL_KEY, repo_url)?;
    set_meta(conn, LAST_BACKUP_KEY, &timestamp.to_string())?;
    if let Some(ref h) = commit_hash {
        set_meta(conn, LAST_COMMIT_KEY, h)?;
    }
    set_meta(conn, LAST_STATUS_KEY, "ok")?;

    let total_files = count_files_in_dir(&backup_dir)?;

    Ok(BackupReport {
        success: true,
        commit_hash,
        total_sessions,
        total_files_backed_up: total_files,
        message,
        timestamp,
    })
}

// ---------------------------------------------------------------------------
// Helpers: Git execution
// ---------------------------------------------------------------------------

fn ensure_git_repo(dir: &Path, repo_url: &str) -> Result<()> {
    let git_dir = dir.join(".git");
    if !git_dir.exists() {
        run_git_cmd(dir, &["init"])?;
        run_git_cmd(dir, &["branch", "-M", "main"])?;
        run_git_cmd(dir, &["remote", "add", "origin", repo_url])?;
    } else {
        // Ensure remote URL is up-to-date
        let _ = run_git_cmd(dir, &["remote", "set-url", "origin", repo_url]);
    }

    // Configure sensible git repo settings for backup
    let _ = run_git_cmd(dir, &["config", "core.autocrlf", "false"]);
    Ok(())
}

fn run_git_cmd(dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .with_context(|| format!("failed to execute git {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ---------------------------------------------------------------------------
// Helpers: Curated Data Export
// ---------------------------------------------------------------------------

fn export_curated_tables(conn: &Connection, dest_dir: &Path) -> Result<()> {
    // 1. Favorites
    let mut fav_stmt = conn.prepare(
        "SELECT session_id, created_at FROM favorites ORDER BY created_at DESC",
    )?;
    let fav_rows: Vec<Value> = fav_stmt
        .query_map([], |r| {
            Ok(json!({
                "session_id": r.get::<_, String>(0)?,
                "created_at": r.get::<_, Option<String>>(1)?,
            }))
        })?
        .filter_map(Result::ok)
        .collect();
    fs::write(
        dest_dir.join("favorites.json"),
        serde_json::to_string_pretty(&fav_rows)?,
    )?;

    // 2. Tags
    let mut tag_stmt = conn.prepare("SELECT session_id, tag FROM tags ORDER BY session_id, tag")?;
    let tag_rows: Vec<Value> = tag_stmt
        .query_map([], |r| {
            Ok(json!({
                "session_id": r.get::<_, String>(0)?,
                "tag": r.get::<_, String>(1)?,
            }))
        })?
        .filter_map(Result::ok)
        .collect();
    fs::write(
        dest_dir.join("tags.json"),
        serde_json::to_string_pretty(&tag_rows)?,
    )?;

    // 3. Notes
    let mut notes_stmt = conn.prepare(
        "SELECT session_id, note, updated FROM notes ORDER BY updated DESC",
    )?;
    let note_rows: Vec<Value> = notes_stmt
        .query_map([], |r| {
            Ok(json!({
                "session_id": r.get::<_, String>(0)?,
                "note": r.get::<_, String>(1)?,
                "updated": r.get::<_, Option<String>>(2)?,
            }))
        })?
        .filter_map(Result::ok)
        .collect();
    fs::write(
        dest_dir.join("notes.json"),
        serde_json::to_string_pretty(&note_rows)?,
    )?;

    // 4. Snippets (KB solutions)
    let mut snip_stmt = conn.prepare(
        "SELECT id, session_id, message_idx, title, lang, content, tags, created_at, tool, role, message_ts FROM snippets ORDER BY created_at DESC",
    )?;
    let snip_rows: Vec<Value> = snip_stmt
        .query_map([], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "session_id": r.get::<_, String>(1)?,
                "message_idx": r.get::<_, i64>(2)?,
                "title": r.get::<_, String>(3)?,
                "lang": r.get::<_, String>(4)?,
                "content": r.get::<_, String>(5)?,
                "tags": r.get::<_, String>(6)?,
                "created_at": r.get::<_, String>(7)?,
                "tool": r.get::<_, Option<String>>(8)?,
                "role": r.get::<_, Option<String>>(9)?,
                "message_ts": r.get::<_, Option<i64>>(10)?,
            }))
        })?
        .filter_map(Result::ok)
        .collect();
    fs::write(
        dest_dir.join("snippets.json"),
        serde_json::to_string_pretty(&snip_rows)?,
    )?;

    // 5. Summaries
    let mut sum_stmt = conn.prepare(
        "SELECT session_id, summary, created, kind FROM summaries ORDER BY session_id",
    )?;
    let sum_rows: Vec<Value> = sum_stmt
        .query_map([], |r| {
            Ok(json!({
                "session_id": r.get::<_, String>(0)?,
                "summary": r.get::<_, String>(1)?,
                "created": r.get::<_, Option<String>>(2)?,
                "kind": r.get::<_, Option<String>>(3)?,
            }))
        })?
        .filter_map(Result::ok)
        .collect();
    fs::write(
        dest_dir.join("summaries.json"),
        serde_json::to_string_pretty(&sum_rows)?,
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers: Web Sessions Export with 45 MB Chunking
// ---------------------------------------------------------------------------

fn export_web_session_files(dest_dir: &Path) -> Result<()> {
    let roots = crate::adapters::web_chat::web_session_roots();
    let mut copied_files = Vec::new();

    for root in roots {
        if !root.exists() {
            continue;
        }
        let entries = match fs::read_dir(&root) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext != "json" {
                continue;
            }

            let file_name = path.file_name().unwrap().to_string_lossy().to_string();
            let size = fs::metadata(&path)?.len();

            if size > MAX_CHUNK_BYTES {
                // Split into chunks <= MAX_CHUNK_BYTES so GitHub will never reject it
                split_file(&path, dest_dir, &file_name)?;
            } else {
                fs::copy(&path, dest_dir.join(&file_name))?;
            }
            copied_files.push(file_name);
        }
    }

    // Write recombine helper scripts
    write_recombine_helpers(dest_dir)?;

    Ok(())
}

fn split_file(src_path: &Path, dest_dir: &Path, base_name: &str) -> Result<usize> {
    let mut src = File::open(src_path)?;
    let mut buf = vec![0u8; MAX_CHUNK_BYTES as usize];
    let mut chunk_idx = 0;

    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let chunk_name = format!("{}.part{:02}", base_name, chunk_idx);
        let mut out = File::create(dest_dir.join(&chunk_name))?;
        out.write_all(&buf[..n])?;
        chunk_idx += 1;
    }

    Ok(chunk_idx)
}

fn write_recombine_helpers(dest_dir: &Path) -> Result<()> {
    // Python script to recombine any .partXX files in the directory
    let py_script = r#"#!/usr/bin/env python3
"""Recombines split .partXX files into their original full-size files."""
import os, glob, re

def recombine():
    parts = glob.glob("*.part00")
    for first_part in parts:
        base_name = first_part[:-7]
        part_files = sorted(glob.glob(f"{base_name}.part*"))
        print(f"Recombining {len(part_files)} parts into {base_name}...")
        with open(base_name, "wb") as out:
            for p in part_files:
                with open(p, "rb") as inp:
                    out.write(inp.read())
        print(f"  ✓ {base_name} restored ({os.path.getsize(base_name)} bytes).")

if __name__ == "__main__":
    recombine()
"#;
    fs::write(dest_dir.join("recombine.py"), py_script)?;

    // PowerShell script to do the same on Windows
    let ps_script = r#"# Recombines split .partXX files into their original full-size files
Get-ChildItem -Filter "*.part00" | ForEach-Object {
    $baseName = $_.Name.Substring(0, $_.Name.Length - 7)
    $parts = Get-ChildItem -Filter "$baseName.part*" | Sort-Object Name
    Write-Host "Recombining $($parts.Count) parts into $baseName..."
    $out = [System.IO.File]::Create((Join-Path $PSScriptRoot $baseName))
    foreach ($p in $parts) {
        $bytes = [System.IO.File]::ReadAllBytes($p.FullName)
        $out.Write($bytes, 0, $bytes.Length)
    }
    $out.Close()
    Write-Host "  ✓ $baseName restored."
}
"#;
    fs::write(dest_dir.join("recombine.ps1"), ps_script)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers: Database Snapshot Export with Compression & Chunking
// ---------------------------------------------------------------------------

fn export_database_snapshot(conn: &Connection, dest_dir: &Path) -> Result<()> {
    let temp_db = dest_dir.join("temp_snapshot.db");
    if temp_db.exists() {
        let _ = fs::remove_file(&temp_db);
    }

    // 1. Safe, consistent SQLite snapshot without stopping writes or locking
    let temp_db_str = temp_db.to_string_lossy().replace('\\', "/");
    conn.execute(&format!("VACUUM INTO '{temp_db_str}'"), [])
        .context("VACUUM INTO failed")?;

    // 2. Compress the snapshot with gzip
    let gz_path = dest_dir.join("index.db.gz");
    {
        let mut src_file = File::open(&temp_db)?;
        let dst_file = File::create(&gz_path)?;
        let mut encoder = GzEncoder::new(dst_file, Compression::default());
        std::io::copy(&mut src_file, &mut encoder)?;
        encoder.finish()?;
    }

    // Clean up temporary uncompressed db
    let _ = fs::remove_file(&temp_db);

    // 3. If compressed file > 45 MB, split into chunks for GitHub
    let gz_size = fs::metadata(&gz_path)?.len();
    if gz_size > MAX_CHUNK_BYTES {
        split_file(&gz_path, dest_dir, "index.db.gz")?;
        // Remove the giant .gz file so only the chunks are kept for Git
        let _ = fs::remove_file(&gz_path);
    }

    // 4. Write restore scripts for the database
    let py_restore = r#"#!/usr/bin/env python3
"""Restores index.db from compressed and chunked parts."""
import os, glob, gzip, shutil

def restore():
    # 1. Recombine parts if chunked
    parts = sorted(glob.glob("index.db.gz.part*"))
    gz_name = "index.db.gz"
    if parts:
        print(f"Recombining {len(parts)} chunks into {gz_name}...")
        with open(gz_name, "wb") as out:
            for p in parts:
                with open(p, "rb") as inp:
                    shutil.copyfileobj(inp, out)
        print("  ✓ index.db.gz reassembled.")

    # 2. Decompress index.db
    if os.path.exists(gz_name):
        print("Decompressing index.db.gz -> index.db...")
        with gzip.open(gz_name, "rb") as f_in:
            with open("index.db", "wb") as f_out:
                shutil.copyfileobj(f_in, f_out)
        print(f"  ✓ index.db restored ({os.path.getsize('index.db')} bytes). Ready to use!")

if __name__ == "__main__":
    restore()
"#;
    fs::write(dest_dir.join("restore.py"), py_restore)?;

    let ps_restore = r#"# Restores index.db from compressed and chunked parts
$parts = Get-ChildItem -Filter "index.db.gz.part*" | Sort-Object Name
$gzName = Join-Path $PSScriptRoot "index.db.gz"
if ($parts.Count -gt 0) {
    Write-Host "Recombining $($parts.Count) chunks into index.db.gz..."
    $out = [System.IO.File]::Create($gzName)
    foreach ($p in $parts) {
        $bytes = [System.IO.File]::ReadAllBytes($p.FullName)
        $out.Write($bytes, 0, $bytes.Length)
    }
    $out.Close()
}

if (Test-Path $gzName) {
    Write-Host "Decompressing index.db.gz -> index.db..."
    $inStream = [System.IO.File]::OpenRead($gzName)
    $gzStream = [System.IO.Compression.GZipStream]::new($inStream, [System.IO.Compression.CompressionMode]::Decompress)
    $outStream = [System.IO.File]::Create((Join-Path $PSScriptRoot "index.db"))
    $gzStream.CopyTo($outStream)
    $outStream.Close()
    $gzStream.Close()
    $inStream.Close()
    Write-Host "  ✓ index.db restored. Ready to use!"
}
"#;
    fs::write(dest_dir.join("restore.ps1"), ps_restore)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers: Manifest and README Generation
// ---------------------------------------------------------------------------

fn query_session_stats(conn: &Connection) -> Result<(i64, BTreeMap<String, i64>)> {
    let total: i64 = conn.query_row("SELECT count(*) FROM files", [], |r| r.get(0))?;

    let mut stmt = conn.prepare(
        "SELECT tool, count(*) FROM files GROUP BY tool ORDER BY count(*) DESC",
    )?;
    let mut stats = BTreeMap::new();
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;

    for row in rows.flatten() {
        stats.insert(row.0, row.1);
    }

    Ok((total, stats))
}

fn write_manifest_and_readme(
    dest_dir: &Path,
    repo_url: &str,
    total_sessions: i64,
    stats: &BTreeMap<String, i64>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let manifest = json!({
        "wikiagents_version": env!("CARGO_PKG_VERSION"),
        "created_at": now,
        "repo_url": repo_url,
        "total_sessions": total_sessions,
        "sessions_by_tool": stats,
    });

    fs::write(
        dest_dir.join("manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    // Generate informative GitHub README
    let mut md = String::new();
    md.push_str("# 🛡️ WikiAgents & SessionWiki Backup Repository\n\n");
    md.push_str(&format!(
        "Автоматический бэкап всех сессий ИИ-агентов и веб-чатов.\n\n\
         - **Дата выгрузки:** `{}`\n\
         - **Всего сессий в бэкапе:** `{}`\n\
         - **Версия WikiAgents:** `{}`\n\n",
        now,
        total_sessions,
        env!("CARGO_PKG_VERSION")
    ));

    md.push_str("## 📊 Статистика по инструментам (все 18 провайдеров)\n\n");
    md.push_str("| Инструмент (Tool) | Количество сессий |\n");
    md.push_str("|---|---:|\n");
    for (tool, count) in stats {
        md.push_str(&format!("| `{tool}` | {count} |\n"));
    }
    md.push_str(&format!("| **ИТОГО** | **{total_sessions}** |\n\n"));

    md.push_str("## 📁 Структура репозитория\n\n");
    md.push_str("- `database/` — Полный сжатый снапшот SQLite базы (`index.db`), содержащий **все сессии всех IDE-агентов (OpenCode, Claude Code, Codex, Antigravity, Qoder, ZCode и др.)**, всю переписку, диффы кода, затронутые файлы и FTS-индекс. Порезан на части по 45 МБ для обхода лимитов GitHub.\n");
    md.push_str("- `curated/` — Человеческие знания в открытом JSON: избранные сессии ⭐, теги, заметки, KB-сниппеты 📌 и саммари.\n");
    md.push_str("- `web_session/` — Исходные JSON-выгрузки веб-чатов (Claude, DeepSeek, Gemini, Qwen, GLM).\n");
    md.push_str("- `manifest.json` — Машинночитаемая сводка бэкапа.\n\n");

    md.push_str("## 🔄 Как восстановить данные на новом компьютере\n\n");
    md.push_str("### Вариант 1 (Автоматически через приложение):\n");
    md.push_str("В приложении WikiAgents откройте окно «Бэкап», укажите URL этого репозитория и нажмите «Синхронизировать / Восстановить».\n\n");
    md.push_str("### Вариант 2 (Вручную из терминала):\n");
    md.push_str("```bash\ncd database && python restore.py\n```\n");
    md.push_str("После этого готовый `index.db` помещается в рабочую папку `data/` приложения WikiAgents.\n");

    fs::write(dest_dir.join("README.md"), md)?;

    Ok(())
}

fn count_files_in_dir(dir: &Path) -> Result<usize> {
    let mut count = 0;
    for entry in walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .flatten()
    {
        if entry.file_type().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    let val: Result<String, _> =
        conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0));
    match val {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO meta(key, value) VALUES(?1, ?2)",
        [key, value],
    )?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    
    #[test]
    fn test_backup_config_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE meta(key TEXT PRIMARY KEY, value TEXT)", []).unwrap();

        let cfg = get_backup_config(&conn).unwrap();
        assert_eq!(cfg.repo_url, "");
        assert_eq!(cfg.last_backup_at, None);

        set_backup_config(&conn, "https://github.com/AlexZander85/wikiagents-backup.git").unwrap();
        let cfg2 = get_backup_config(&conn).unwrap();
        assert_eq!(cfg2.repo_url, "https://github.com/AlexZander85/wikiagents-backup.git");
    }

    #[test]
    fn test_test_connection_empty_url() {
        let res = test_connection("");
        assert!(!res.ok);
        assert!(res.message.contains("не указан"));
    }
}
