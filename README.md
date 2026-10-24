<div align="center">

# AgentWiki-rs 🧠⚡

**The Universal Local Memory & Desktop Command Center for AI Coding Agents.**  
*Single self-contained ~3.8 MB binary written in 100% Rust. Zero dependencies. Zero cloud. Zero Electron.*

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](https://github.com/AlexZander85/AgentWiki-rs)
[![Binary Size](https://img.shields.io/badge/Binary%20Size-~3.8%20MB-brightgreen.svg)]()
[![Language](https://img.shields.io/badge/Language-100%25%20Rust-orange.svg)]()
[![Privacy](https://img.shields.io/badge/Privacy-100%25%20Local%20%26%20Offline-success.svg)]()

[English](README.md) • [Русский](README.ru.md)

</div>

> [!NOTE]
> **Lineage & Mission**: **AgentWiki-rs** is an enhanced standalone fork based on the excellent work of [youdie006/sessionwiki](https://github.com/youdie006/sessionwiki). While the upstream project focuses strictly on a CLI and web browser viewer, **AgentWiki-rs** evolves the codebase into a single self-contained desktop application with a native OS GUI (`wry`/WebView2), deep GUI usability refinements, long-term agent memory via MCP, server-side sorting, lazy streaming pagination, and major performance optimizations.

---

## ⚡ What is AgentWiki-rs?

Every day, coding agents like **Claude Code, Codex, Antigravity, OpenCode, Qoder, ZCode, Cline, and DeepSeek** generate thousands of lines of code, architectural decisions, and bug investigations. By default, each tool stores its history in disparate, siloed formats across hidden directories. When tools prune history or when you switch agents, valuable context and solutions vanish.

**AgentWiki-rs** unifies all your AI coding sessions into a single, blazing-fast, durable local database. It is delivered as a **single, dependency-free binary** containing:
1. **A Native Desktop GUI** (powered by native OS WebViews &mdash; no heavy Electron or Chromium bundled).
2. **One-Click & Scheduled Background Sync** (manual `🔄 Sync` button with spinner + background auto-sync every 10 minutes).
3. **⭐ Favorites & Starred Sessions** (star crucial tasks, filter instantly with `⭐ Favorites (N)`, query via CLI & MCP).
4. **Durable Agent Memory via MCP** (`save_memory` & `list_snippets` for Claude, Codex, Cursor, and Antigravity to remember architectural decisions across turns).
5. **Microsecond Full-Text Search (FTS5 + Trigram)** across millions of messages and code changes.
6. **Curated Knowledge Base & Bookmarks** with multi-filter sorting and language detection.
7. **Activity Heatmap & Agent Analytics** (yearly contribution graph, hourly breakdown, agent share).
8. **Code Provenance & AI Blame** tracing any modified line of code back to the conversation that created it.

---

## 🌟 Key Highlights

### 🚀 100% Rust & Single Tiny Executable (~3.8 MB)
- **Zero runtime dependencies**: No Node.js, no Python, no Electron.
- **Instant startup** (< 20ms) and minimal RAM usage (< 35 MB).
- Compile once, run anywhere: Windows (`agentwiki-app.exe`), macOS, and Linux.

### 🔄 One-Click Sync & Background Scheduled Auto-Sync
- **Manual Sync (`🔄 Sync`)**: Real-time button right above the session list with animated spinner and `Last synced HH:MM` timestamp.
- **Automated Periodic Sync (`Auto-sync (10m)`)**: Runs silently in the background every 10 minutes, indexing newly created agent turns without interrupting your workflow.
- **Persistent State**: Auto-sync preferences and timestamps are saved in `localStorage`.

### ⭐ Starred Sessions & Favorites Management
- **One-Click Star / Unstar (`⭐`)**: Bookmark important debugging sessions, complex refactors, and architectural discussions.
- **Instant Filter Chip**: Dedicated `⭐ Favorites (N)` quick-filter chip in the sidebar to isolate starred work with zero typing.
- **Context Menu & CLI / MCP**: Right-click context menu (`Add to favorites`), CLI command (`agentwiki favorites`), and MCP tool (`list_favorites`).

### 🤖 18+ Agent Adapters Out of the Box
Automatically locates and indexes sessions from all major coding agents:
- **Anthropic Claude Code** (`~/.claude/projects`)
- **OpenAI Codex** (`~/.codex/sessions` & `thread_history_1.sqlite`)
- **OpenCode** (both **v1.x** SQLite/JSON and **v2.x** `session_v2` schemas)
- **Google Antigravity IDE & CLI (`agy`)** (`~/.gemini/antigravity` & `~/.gemini/antigravity-cli`)
- **ZCode** (`~/.zcode/cli`)
- **Qoder** (`AppData/Roaming/com.qoder.app.stable`)
- **DeepSeek Harness** (`~/.dsh/sessions`)
- **Kimi Code** (`~/.kimi-code/sessions`)
- **Hermes Agent** (`~/.hermes/state.db`)
- **Cline, Roo Code, Kilo Code, Gajae Code** (`globalStorage`)
- **Aider, Continue.dev, Gemini CLI, Prodex, gptme**, and more (21 adapter configurations in total).

### 🧠 Model Context Protocol (MCP) Durable Memory
Empower your agents to remember solutions between independent tasks.
- **`save_memory`**: Coding agents can store architectural invariants, debugging conclusions, and configurations directly into your local database.
- **`list_snippets`**: Agents recall past solutions filtered by programming language, tags, or role (`assistant_memory`).
- **`list_favorites`**: Agents can query only your starred, highest-priority sessions for authoritative context.

### 📊 Activity Heatmap & Analytics
- **GitHub-style Contribution Graph**: 52-week heatmap showing session frequency and intensity with full Dark Mode support.
- **24-Hour Circadian Distribution**: See when you and your agents are most active.
- **Agent Market Share**: Visual breakdown of sessions handled by Claude vs Codex vs Antigravity vs OpenCode.
- **Most Touched Files**: Top modified files across your entire project history.

### 📌 Bookmarks & Solution Snippets
- Pin valuable code blocks, configurations, and prompts into a durable local knowledge base.
- Filter by agent tool, message role (`assistant_memory`, `user`, `assistant`, `tool`), or date.
- Sort by created time, original message date, relevance, title, or length.
- Double-click to rename; one-click jump back to the exact turn in the original transcript.

### ↩️ Rewound Turns & Compaction History
- AI sessions frequently rewind or compact context when token limits are reached.
- AgentWiki-rs preserves superseded turns with an interactive **"↩ Rewound"** toggle in the viewer.

---

## 🚀 Quick Start

### 1. Download Prebuilt Executable
Grab the latest release from the [Releases](https://github.com/AlexZander85/AgentWiki-rs) page:
- **Windows**: `agentwiki-app.exe` (or `agentwiki.exe` for CLI-only)
- **macOS / Linux**: `agentwiki-app` / `agentwiki`

*No installer or prerequisites needed &mdash; just double-click and run!*

### 2. Launch Native Desktop App
```bash
./agentwiki-app
```
AgentWiki-rs automatically scans your local disks, discovers existing AI coding sessions, indexes them in parallel, and opens the native dashboard.

### 3. Connect via MCP (Claude Desktop, Cursor, Antigravity)
Add AgentWiki-rs to your MCP config (`claude_desktop_config.json` or `cursor.json`):
```json
{
  "mcpServers": {
    "agentwiki": {
      "command": "agentwiki",
      "args": ["mcp"]
    }
  }
}
```

Now your agents have direct, read-and-write access to your local knowledge base using:
- `search_sessions(query)` &mdash; search past solutions across all tools.
- `save_memory(title, content, tags)` &mdash; save architectural decisions.
- `list_snippets(role="assistant_memory")` &mdash; retrieve durable memories.
- `list_favorites()` &mdash; retrieve starred reference sessions.

---

## 💻 CLI Commands

AgentWiki-rs includes a comprehensive CLI alongside the desktop GUI:

```bash
# Scan and report disk space used by AI coding agents
agentwiki scan

# Manually trigger index sync across all agent stores
agentwiki sync

# List starred / favorite sessions
agentwiki favorites --json

# Full-text search across all agent conversations
agentwiki search "JWT authentication with RS256"

# Trace which AI session created or modified a specific file
agentwiki trace src/auth/jwt.rs

# Git blame with session attribution
agentwiki blame src/auth/jwt.rs

# Start the local browser server (optional)
agentwiki web --port 7575

# Optimize FTS5 search index and reclaim disk space
agentwiki vacuum

# Self-check health and discoverable agent stores
agentwiki doctor
```

---

## 🏛️ High-Performance Architecture: Why Native OS WebView?

AgentWiki-rs delivers a fluid, instant UI while maintaining a microscopic ~3.8 MB binary footprint and ~35 MB RAM footprint by using an optimal division of labor between Rust and the OS:

### 1. Zero-Electron Native OS WebView (`wry` + `tao`)
- **No Bundled Chromium / Node.js**: Unlike Electron apps (which weigh 120–180 MB and consume 200+ MB RAM), AgentWiki-rs leverages the operating system's native hardware-accelerated web engine:
  - **Windows**: Microsoft Edge WebView2 (preinstalled in Windows 10/11, GPU DirectComposition).
  - **macOS**: Apple WebKit (`WKWebView` with Metal acceleration on Apple Silicon & Intel).
  - **Linux**: WebKitGTK (`libwebkit2gtk-4.1`).
- **Native OS Font Smoothing & Anti-Aliasing**: Sub-pixel text rendering, crisp font ligatures (`Fira Code`, `JetBrains Mono`), emoji fallback, and seamless multi-monitor DPI scaling.
- **Pure Native CLI/MCP Mode**: When invoked via CLI (`agentwiki search`) or MCP by AI agents, the GUI framework is not even loaded, executing in pure Rust with ~10 MB RAM.

### 2. Client-Side Markdown with Lazy Rendering vs. Server-Side Pre-Rendering
Why parse Markdown via `markdown-it` in the WebView rather than pre-generating HTML in Rust?
- **3x–5x Smaller Payloads**: Rust streams compact raw Markdown JSON directly from memory-mapped SQLite (`mmap`). Sending pre-rendered HTML would bloat payloads from 150 KB to 2.5 MB due to thousands of `<pre>`, `<code>`, and syntax `<span>` tags, degrading serialization and IPC throughput.
- **Zero-Lag Lazy Loading (`IntersectionObserver`)**: Opening a session with 500+ messages renders only the first 30 visible turns eagerly (< 15 ms). Off-screen messages are parsed on-demand as you scroll, completely eliminating DOM layout thrashing.
- **Safe Dynamic Highlighting & Interaction**: Search keyword highlighting (`<mark>`), one-click code copy badges, foldable tool executions, and loss-free "Copy as Markdown" are executed natively within the DOM without risk of corrupting server-side HTML entity trees.

---

## 🔒 Privacy & Local Security

- **100% Local**: No telemetry, no tracking, no cloud dependencies. Your data never leaves your workstation.
- **Memory-Mapped SQLite (mmap)**: Reads multi-gigabyte session archives in microseconds with zero CPU overhead.

---

## 🙏 Acknowledgments & Credits

AgentWiki-rs stands on the shoulders of open-source excellence:
- **[youdie006/sessionwiki](https://github.com/youdie006/sessionwiki)** &mdash; Sincere gratitude to the author of the original project for the rock-solid Rust foundation, the elegant SQLite FTS5 architecture, and the extensible adapter design for integrating diverse AI coding tools.
- **[markdown-it](https://github.com/markdown-it/markdown-it)** &mdash; Huge thanks to the authors and maintainers of markdown-it for the lightning-fast, highly extensible, and rock-solid Markdown parser and rendering engine.

---

## 📄 License

Licensed under the [MIT License](LICENSE).
