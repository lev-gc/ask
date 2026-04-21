use super::openai::OpenAI;
use super::{ChatRequest, Provider};
use crate::config::ProviderConfig;
use anyhow::{anyhow, Context, Result};
use futures_util::stream::BoxStream;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Copilot {
    cfg: ProviderConfig,
}

impl Copilot {
    pub fn from_config(cfg: &ProviderConfig) -> Result<Self> {
        Ok(Self { cfg: cfg.clone() })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct CachedToken {
    token: String,
    expires_at: u64,
    endpoint: Option<String>,
}

fn cache_path() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("", "", "ask")
        .ok_or_else(|| anyhow!("cannot resolve cache dir"))?;
    let dir = dirs.cache_dir().to_path_buf();
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("copilot_token.json"))
}

fn read_gh_oauth_token() -> Result<String> {
    let home = std::env::var("HOME").map_err(|_| anyhow!("HOME not set"))?;
    // Try several known paths used by VSCode/JetBrains Copilot installs.
    let candidates = [
        format!("{}/.config/github-copilot/hosts.json", home),
        format!("{}/.config/github-copilot/apps.json", home),
    ];
    for path in candidates.iter() {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(obj) = v.as_object() {
                    for (_key, entry) in obj {
                        if let Some(t) = entry.get("oauth_token").and_then(|s| s.as_str()) {
                            return Ok(t.to_string());
                        }
                    }
                }
            }
        }
    }
    anyhow::bail!(
        "Copilot GitHub oauth token not found. Sign in via the GitHub Copilot extension (VSCode/JetBrains) first, or set api_key = <token> in [providers.copilot]."
    )
}

async fn fetch_session_token() -> Result<CachedToken> {
    let cache = cache_path()?;
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    if let Ok(text) = std::fs::read_to_string(&cache) {
        if let Ok(cached) = serde_json::from_str::<CachedToken>(&text) {
            if cached.expires_at > now + 60 {
                return Ok(cached);
            }
        }
    }
    let gh_token = read_gh_oauth_token()?;
    let client = reqwest::Client::new();
    let resp = client
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Authorization", format!("token {}", gh_token))
        .header("User-Agent", "GithubCopilot/1.155.0")
        .header("Editor-Version", "vscode/1.90.0")
        .header("Editor-Plugin-Version", "copilot-chat/0.17.0")
        .send()
        .await
        .context("fetching copilot session token")?
        .error_for_status()?;
    let v: serde_json::Value = resp.json().await?;
    let token = v
        .get("token")
        .and_then(|s| s.as_str())
        .ok_or_else(|| anyhow!("copilot token response missing `token`"))?
        .to_string();
    let expires_at = v.get("expires_at").and_then(|s| s.as_u64()).unwrap_or(now + 1500);
    let endpoint = v
        .get("endpoints")
        .and_then(|e| e.get("api"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    let cached = CachedToken {
        token,
        expires_at,
        endpoint,
    };
    let _ = std::fs::write(&cache, serde_json::to_string(&cached)?);
    Ok(cached)
}

#[async_trait::async_trait]
impl Provider for Copilot {
    async fn chat(&self, req: ChatRequest<'_>) -> Result<BoxStream<'static, Result<String>>> {
        let session = fetch_session_token().await?;
        let base = session
            .endpoint
            .clone()
            .unwrap_or_else(|| "https://api.githubcopilot.com".to_string());
        let mut headers = HeaderMap::new();
        let add = |h: &mut HeaderMap, k: &'static str, v: &str| -> Result<()> {
            h.insert(HeaderName::from_static(k), HeaderValue::from_str(v)?);
            Ok(())
        };
        add(&mut headers, "editor-version", "vscode/1.90.0")?;
        add(
            &mut headers,
            "editor-plugin-version",
            "copilot-chat/0.17.0",
        )?;
        add(&mut headers, "copilot-integration-id", "vscode-chat")?;
        add(&mut headers, "openai-intent", "conversation-panel")?;
        add(&mut headers, "user-agent", "GithubCopilot/1.155.0")?;

        for (k, v) in &self.cfg.extra_headers {
            let name = HeaderName::from_bytes(k.as_bytes())?;
            headers.insert(name, HeaderValue::from_str(v)?);
        }

        let inner = OpenAI::new_with(
            reqwest::Client::new(),
            base,
            session.token,
            headers,
        );
        inner.chat(req).await
    }
}
