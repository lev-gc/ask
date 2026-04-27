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
- **Multi-provider**: Support for Anthropic / Gemini / OpenAI / Kimi / DeepSeek / GitHub Copilot, easily configurable.
- **Environment-aware**: Probes distribution, shell, sudo, and tool availability on demand, ensuring commands fit your system perfectly.
- **Side-effect free**: Never executes automatically, just prints. Optional keypress to copy to clipboard.

On first run, `ask` writes a template to `~/.config/ask/config.toml`. Edit it to set your default provider and API key (or the corresponding environment variable).

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

[providers.anthropic]
kind = "anthropic"
base_url = "https://api.anthropic.com/v1"
api_key_env = "ANTHROPIC_API_KEY"
model = "claude-haiku-4-5-20251001"

[providers.gemini]
kind = "gemini"
base_url = "https://generativelanguage.googleapis.com/v1beta"
api_key_env = "GEMINI_API_KEY"
model = "gemini-2.5-flash"

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

[providers.copilot]
kind = "copilot"
model = "gpt-4o-mini"

```

- `kind` must be `anthropic`, `gemini`, `openai`, or `copilot`. For custom OpenAI-compatible interfaces (local Ollama, vLLM, other clouds), use `kind = "openai"` and change the `base_url`.
- API key field — pick **one** per provider:
  - `api_key` — the literal key as a string (plaintext on disk, not recommended; the config file is auto-chmod'd to 0600).
  - `api_key_env` — the **name of the environment variable** that holds the key (e.g. `"OPENAI_API_KEY"`). `ask` calls `std::env::var(<that name>)` at runtime; do **not** put the key itself here.
  - If both are set, `api_key` wins.
- Provider priority: `--provider` > `$ASK_PROVIDER` > `default_provider`.
- Model override: `--model` > `model` inside the provider config.

### GitHub Copilot Special Note

Copilot has no public chat API; this tool uses the identical internal flow as the VSCode Copilot Chat extension:

1. Reads GitHub oauth token from `~/.config/github-copilot/hosts.json` (or `apps.json`) (requires prior login via VSCode/JetBrains extension).
2. Exchanges it for a short-lived session token via `https://api.github.com/copilot_internal/v2/token`.
3. Caches it in `~/.cache/ask/copilot_token.json` and reuses until expiration.
4. Sends OpenAI-compatible requests to `https://api.githubcopilot.com/chat/completions`.

Because this is a reverse-engineered endpoint, **stability is not guaranteed**, GitHub may change it at any time.
