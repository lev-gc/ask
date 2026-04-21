# ask

Fast AI-powered Linux command assistant. Describe what you want to do; `ask` prints the right command(s) with a one-line explanation, tuned to your system — it does **not** execute anything.

```sh
$ ask check disk usage
$ df -h
Check disk usage in human-readable format.

[c] copy first command   [any] quit
```

---

## Design Goals

- **Fast**: Cold start < 10ms, time to first byte depends only on model TTFT.
- **Minimalist**: Outputs only the command + a one-line explanation, no fluff.
- **Multi-provider**: Support for OpenAI / Anthropic / Kimi / DeepSeek / GitHub Copilot, easily configurable.
- **Environment-aware**: Probes distribution, shell, sudo, and tool availability on demand, ensuring commands fit your system perfectly.
- **Side-effect free**: Never executes automatically, just prints. Optional keypress to copy to clipboard.

## Installation

Requires Rust 1.75+ (uses `std::io::IsTerminal`).

```bash
git clone <repo> ask && cd ask
cargo install --path .           # Installs to ~/.cargo/bin/ask
# OR
cargo build --release && cp target/release/ask ~/.local/bin/
```

On first run, it will write a template to `~/.config/ask/config.toml`. Edit it to set your default provider and API key (or the corresponding environment variable).

## Usage

```bash
ask check current disk usage               # Default provider
ask -p kimi make nginx start on boot       # Switch provider
ask -m gpt-4o clean logs older than 30 days # Override model
ask -e full give me a deployment command   # Extended environment probing
ask -e tools clean docker cache            # Add tool availability probing
some-cmd 2>&1 | ask why did this fail      # Pipe input as context
ask --no-stream batch query                # Disable streaming
```

**Environment Probing Levels** (`-e` / `--env`, default `min`):

| Level | Content                                                | Time   |
|-------|--------------------------------------------------------|--------|
| min   | OS + Kernel + Shell + root status                      | < 2ms  |
| full  | min + User + passwordless sudo + git repo status + cwd | ~5ms   |
| tools | full + common CLI tools (apt/dnf/docker/kubectl etc.)  | ~20ms  |

**Post-output Interaction** (TTY only): Press `c` to copy the first command to your clipboard. Any other key exits immediately. Skipped when output is piped.

## Configuration

Default path: `~/.config/ask/config.toml` (permissions auto-set to 0600; throws a stderr warning if insecure).

```toml
default_provider = "kimi"

[providers.openai]
kind = "openai"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
model = "gpt-4o-mini"

[providers.kimi]
kind = "openai"                              # Reuses OpenAI protocol
base_url = "https://api.moonshot.cn/v1"
api_key_env = "MOONSHOT_API_KEY"
model = "moonshot-v1-8k"

[providers.deepseek]
kind = "openai"
base_url = "https://api.deepseek.com/v1"
api_key_env = "DEEPSEEK_API_KEY"
model = "deepseek-chat"

[providers.anthropic]
kind = "anthropic"
base_url = "https://api.anthropic.com/v1"
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-haiku-4-5-20251001"

[providers.copilot]
kind = "copilot"
model = "gpt-4o-mini"
```

- `kind` must be `openai`, `anthropic`, or `copilot`. For custom OpenAI-compatible interfaces (local Ollama, vLLM, other clouds), use `kind = "openai"` and change the `base_url`.
- Key resolution priority: `api_key` (plaintext, not recommended) > `api_key_env` environment variable.
- Provider priority: `--provider` > `$ASK_PROVIDER` > `default_provider`.
- Model override: `--model` > `model` inside the provider config.

### GitHub Copilot Special Note

Copilot has no public chat API; this tool uses the identical internal flow as the VSCode Copilot Chat extension:

1. Reads GitHub oauth token from `~/.config/github-copilot/hosts.json` (or `apps.json`) (requires prior login via VSCode/JetBrains extension).
2. Exchanges it for a short-lived session token via `https://api.github.com/copilot_internal/v2/token`.
3. Caches it in `~/.cache/ask/copilot_token.json` and reuses until expiration.
4. Sends OpenAI-compatible requests to `https://api.githubcopilot.com/chat/completions`.

Because this is a reverse-engineered endpoint, **stability is not guaranteed**, GitHub may change it at any time.

---

## Source Code Structure

```
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
        ├── openai.rs       # OpenAI chat/completions (Kimi/DeepSeek/Custom)
        ├── anthropic.rs    # Anthropic /v1/messages
        └── copilot.rs      # GH Copilot token exchange + reuse openai.rs
```

### Module Responsibilities

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
