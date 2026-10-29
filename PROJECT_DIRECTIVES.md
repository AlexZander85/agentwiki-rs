# PROJECT DIRECTIVES

## Commands & Build
- Build: `cargo build` / `cargo build --release --bin wikiagents`
- Test: `cargo test --lib` (214+ tests)
- CLI run: `cargo run --bin wikiagents -- scan`

## Invariants & Conventions
- All adapters implement `Adapter` trait in `src/adapters/` and register in `adapters::all()`.
- Shared stores (SQLite) use `store()` and `parse_key()`. File-per-session tools use `discover()` and `parse()`.
- `Message::tool_call` creates a message with `role: Role::Tool`.
- Redaction and truncation: run `redact` before truncation; never split UTF-8 boundaries.
