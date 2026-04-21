config.rsuse anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootConfig {
    pub default_provider: String,
    pub providers: BTreeMap<String, ProviderConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// openai | anthropic | copilot
    #[serde(default = "default_kind")]
    pub kind: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub model: String,
    #[serde(default)]
    pub extra_headers: BTreeMap<String, String>,
}

fn default_kind() -> String {
    "openai".into()
}

impl ProviderConfig {
    pub fn resolve_api_key(&self) -> Option<String> {
        if let Some(k) = &self.api_key {
            if !k.is_empty() {
                return Some(k.clone());
            }
        }
        if let Some(env_name) = &self.api_key_env {
            if let Ok(v) = std::env::var(env_name) {
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
        None
    }
}

pub fn default_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "ask")
        .ok_or_else(|| anyhow!("cannot determine config directory"))?;
    Ok(dirs.config_dir().join("config.toml"))
}

pub fn load(explicit: Option<&Path>) -> Result<(RootConfig, PathBuf)> {
    let path = match explicit {
        Some(p) => p.to_path_buf(),
        None => default_path()?,
    };
    if !path.exists() {
        write_template(&path)?;
        return Err(anyhow!(
            "no config found — wrote template to {}. edit it and set an API key (or the matching env var), then run again.",
            path.display()
        ));
    }
    check_permissions(&path);
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading config {}", path.display()))?;
    let cfg: RootConfig =
        toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))?;
    Ok((cfg, path))
}

fn check_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mode = meta.permissions().mode() & 0o777;
            if mode & 0o077 != 0 {
                eprintln!(
                    "warning: {} is world/group readable (mode {:o}). run: chmod 600 {}",
                    path.display(),
                    mode,
                    path.display()
                );
            }
        }
    }
}

pub fn resolve_provider<'a>(
    cfg: &'a RootConfig,
    override_name: Option<&str>,
) -> Result<(&'a str, &'a ProviderConfig)> {
    let name = override_name
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ASK_PROVIDER").ok())
        .unwrap_or_else(|| cfg.default_provider.clone());
    let p = cfg
        .providers
        .get(&name)
        .ok_or_else(|| anyhow!("provider `{}` not found in config", name))?;
    // return a &str that lives as long as cfg
    let key = cfg
        .providers
        .keys()
        .find(|k| *k == &name)
        .expect("just found above");
    Ok((key.as_str(), p))
}

fn write_template(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let template = r#"# ask config — set a default_provider and fill in api_key or api_key_env.
default_provider = "kimi"

[providers.openai]
kind = "openai"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
model = "gpt-4o-mini"

[providers.kimi]
kind = "openai"
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
"#;
    std::fs::write(path, template)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
