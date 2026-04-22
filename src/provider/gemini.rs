use super::{ChatRequest, Provider};
use crate::config::ProviderConfig;
use crate::stream::sse_events;
use anyhow::{anyhow, Context, Result};
use futures_util::stream::{BoxStream, StreamExt};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use serde_json::{json, Value};

pub struct Gemini {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    extra_headers: HeaderMap,
}

impl Gemini {
    pub fn from_config(cfg: &ProviderConfig) -> Result<Self> {
        let base_url = cfg
            .base_url
            .clone()
            .unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string());
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
impl Provider for Gemini {
    async fn chat(&self, req: ChatRequest<'_>) -> Result<BoxStream<'static, Result<String>>> {
        let action = if req.stream {
            "streamGenerateContent?alt=sse"
        } else {
            "generateContent"
        };
        let url = format!("{}/models/{}:{}", self.base_url, req.model, action);

        let body = json!({
            "systemInstruction": {
                "parts": [{"text": req.system}]
            },
            "contents": [
                {"role": "user", "parts": [{"text": req.user}]}
            ],
            "generationConfig": {
                "temperature": 0.2
            }
        });

        let mut rb = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .header("x-goog-api-key", &self.api_key);
        for (k, v) in self.extra_headers.iter() {
            rb = rb.header(k.clone(), v.clone());
        }

        if !req.stream {
            let resp = rb
                .json(&body)
                .send()
                .await
                .context("sending gemini request")?
                .error_for_status()?;
            let v: Value = resp.json().await?;
            let text = extract_text(&v);
            return Ok(Box::pin(futures_util::stream::once(async move { Ok(text) })));
        }

        let resp = rb
            .json(&body)
            .send()
            .await
            .context("sending gemini request")?
            .error_for_status()?;
        let byte_stream = resp.bytes_stream();
        let events = sse_events(byte_stream);
        let text_stream = events.filter_map(|ev| async move {
            match ev {
                Err(e) => Some(Err(e)),
                Ok(ev) => match serde_json::from_str::<Value>(&ev.data) {
                    Err(_) => None,
                    Ok(v) => {
                        let text = extract_text(&v);
                        if text.is_empty() {
                            None
                        } else {
                            Some(Ok(text))
                        }
                    }
                },
            }
        });
        Ok(Box::pin(text_stream))
    }
}

fn extract_text(v: &Value) -> String {
    let Some(parts) = v
        .get("candidates")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("content"))
        .and_then(|c| c.get("parts"))
        .and_then(|p| p.as_array())
    else {
        return String::new();
    };
    let mut out = String::new();
    for p in parts {
        if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
            out.push_str(t);
        }
    }
    out
}
