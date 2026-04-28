# Development Guide

This document describes how to build `ask` from source, the internal layout, how to extend it with new providers, and the performance budget the codebase aims to stay within.

## Installation (Build from Source)

Requires Rust 1.75+ (uses `std::io::IsTerminal`).

```bash
git clone <repo> ask && cd ask
cargo install --path .           # Installs to ~/.cargo/bin/ask
# OR
cargo build --release && cp target/release/ask ~/.local/bin/
```

## Source Code Structure

```tree
ask/
├── Cargo.toml              # Dependencies, release profile (lto=fat, opt-level=z, strip)
└── src/
    ├── main.rs             # Entry: arg parsing, stdin reading, orchestration
    ├── cli.rs              # clap derive definitions
    ├── config.rs           # TOML load, env overrides, templating, permission checks
    ├── env_probe.rs        # Env probe levels, prompt formatting
    ├── prompt.rs           # SYSTEM prompt constraints + user message assembly
    ├── stream.rs           # SSE line parser (shared across providers)
    ├── render.rs           # Stream renderer: $ prefix highlights, remembers first cmd
    ├── interact.rs         # Post-run single-key copy (crossterm + arboard)
    └── provider/
        ├── mod.rs          # Provider trait + build()
        ├── anthropic.rs    # Anthropic /v1/messages
        ├── gemini.rs       # Google Gemini generateContent / streamGenerateContent
        ├── openai.rs       # OpenAI chat/completions (Kimi/DeepSeek/Custom)
        └── copilot.rs      # GH Copilot token exchange + reuse openai.rs
```

## Module Responsibilities

**`main.rs`** — Synchronous main flow: parse CLI → read stdin if non-TTY → probe env → assemble prompt → load config → build provider → stream response in single-threaded tokio runtime → render → interact before exit. Single-threaded runtime skips multi-thread startup latency.
**`cli.rs`** — `clap` derivations. `question: Vec<String>` + `trailing_var_arg` + `allow_hyphen_values` easily absorbs arbitrary raw text.
**`config.rs`** — Config structs & resolution. First run dumps 0600 template via `write_template()`.
**`env_probe.rs`** — Structured `EnvInfo`. Embeds `key=value` outputs into prompt `<env>` blocks. **Key design**: Avoids `fork`ing sub-processes (except `sudo -n true`), directly reads `/etc/os-release`, `/proc/sys/kernel/osrelease`, and scans `$PATH` to stay within microseconds.
**`prompt.rs`** — Strict `SYSTEM` constraints ensure outputs match `$ cmd\n<explanation>` repeatedly, enabling stateless stream parsing in `render.rs`.
**`stream.rs`** — Generic `sse_events()` taking `Stream<Bytes>` -> `Stream<SseEvent>`.
**`render.rs`** — Incremental buffer rendering. `$ ` lines formatted in bright cyan, others dimmed. Captures `first_cmd`. ANSI formats enabled only for TTYs.
**`interact.rs`** — Single-keypress collection via `crossterm`. Copies to clipboard with `arboard`.
**`provider/*`** — Independent API formatters. `copilot` intelligently delegates down to `openai`.

## Adding a New Provider

### Case 1: OpenAI-compatible (Most Common)

No code changes needed, just add to config:

```toml
[providers.my-local]
kind = "openai"
base_url = "http://localhost:11434/v1"      # Ollama
api_key_env = "OLLAMA_API_KEY"              # Can be empty
model = "llama3.1:8b"
```

### Case 2: Custom Protocol

1. Create `src/provider/foo.rs`, define `struct Foo`, `impl Provider`.
2. Add a branch in `src/provider/mod.rs` `build()`: `"foo" => Box::new(foo::Foo::from_config(cfg)?)`.
3. Try reusing `stream::sse_events` for streaming parsing.

## Development Workflow

```bash
cargo check                   # Fast type check
cargo build                   # Debug build
cargo build --release         # Release build (~50s cold, 5s incremental)
cargo clippy -- -D warnings   # Lint
cargo fmt                     # Format
```

**Pre-release Checklist**:

```bash
ls -lh target/release/ask                   # Expect < 3MB
time ./target/release/ask --help            # Expect < 10ms
./target/release/ask check memory 2>&1 | head # Test a real output
```

## Performance Budget

| Phase                      | Budget     | Current                 |
|----------------------------|------------|-------------------------|
| Process Start + CLI Parse  | < 5ms      | ~2ms                    |
| Env Probe (min)            | < 2ms      | < 1ms                   |
| Env Probe (tools)          | < 30ms     | ~5ms (no fork)          |
| HTTP TTFB                  | Network RTT  | Depends on Provider     |
| Per-chunk Render           | < 50µs     | Buffering + ANSI concat |
| Binary Size                | < 3MB      | 2.7MB                   |

**Deliberately excluded** (to preserve raw speed): tokenizers, embeddings, SQLite/disk-caching, heavy regex engines (using `strip_prefix` / `find` instead), multi-threaded runtimes, OpenSSL (using `rustls`).
