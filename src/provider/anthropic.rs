use super::{ChatRequest, Provider};
use crate::config::ProviderConfig;
use crate::stream::sse_events;
use anyhow::{anyhow, Context, Result};
use futures_util::stream::{BoxStream, StreamExt};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use serde_json::{json, Value};

pub struct Anthropic {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    extra_headers: HeaderMap,
}

impl Anthropic {
    pub fn from_config(cfg: &ProviderConfig) -> Result<Self> {
        let base_url = cfg
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());
        let api_key = cfg
            .resolve_api_key()
            .ok_or_else(|| anyhow!("no api key set (check api_key or api_key_env)"))?;
        let mut headers = HeaderMap::new();
        for (k, v) in &cfg.extra_headers {
            let name = HeaderName::from_bytes(k.as_bytes())?;
            let val = HeaderValue::from_str(v)?;
            headers.insert(name, val);
        }
        Ok(Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            extra_headers: headers,
        })
    }
}

#[async_trait::async_trait]
impl Provider for Anthropic {
    async fn chat(&self, req: ChatRequest<'_>) -> Result<BoxStream<'static, Result<String>>> {
        let url = format!("{}/messages", self.base_url);
        let body = json!({
            "model": req.model,
            "max_tokens": 1024,
            "stream": req.stream,
            "system": req.system,
            "messages": [
                {"role": "user", "content": req.user}
            ],
            "temperature": 0.2
        });

        let mut rb = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01");
        for (k, v) in self.extra_headers.iter() {
            rb = rb.header(k.clone(), v.clone());
        }

        if !req.stream {
            let resp = rb
                .json(&body)
                .send()
                .await
                .context("sending anthropic request")?
                .error_for_status()?;
            let v: Value = resp.json().await?;
            let text = v
                .get("content")
                .and_then(|c| c.get(0))
                .and_then(|c| c.get("text"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();
            return Ok(Box::pin(futures_util::stream::once(async move { Ok(text) })));
        }

        let resp = rb
            .json(&body)
            .send()
            .await
            .context("sending anthropic request")?
            .error_for_status()?;
        let byte_stream = resp.bytes_stream();
        let events = sse_events(byte_stream);
        let text_stream = events.filter_map(|ev| async move {
            match ev {
                Err(e) => Some(Err(e)),
                Ok(ev) => {
                    if ev.event != "content_block_delta" {
                        return None;
                    }
                    match serde_json::from_str::<Value>(&ev.data) {
                        Err(_) => None,
                        Ok(v) => v
                            .get("delta")
                            .and_then(|d| d.get("text"))
                            .and_then(|c| c.as_str())
                            .filter(|s| !s.is_empty())
                            .map(|s| Ok(s.to_string())),
                    }
                }
            }
        });
        Ok(Box::pin(text_stream))
    }
}
