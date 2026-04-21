use anyhow::Result;
use futures_util::stream::BoxStream;

pub mod anthropic;
pub mod copilot;
pub mod openai;

pub struct ChatRequest<'a> {
    pub system: &'a str,
    pub user: &'a str,
    pub model: &'a str,
    pub stream: bool,
}

#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    async fn chat(&self, req: ChatRequest<'_>) -> Result<BoxStream<'static, Result<String>>>;
}

pub fn build(cfg: &crate::config::ProviderConfig) -> Result<Box<dyn Provider>> {
    match cfg.kind.as_str() {
        "openai" => Ok(Box::new(openai::OpenAI::from_config(cfg)?)),
        "anthropic" => Ok(Box::new(anthropic::Anthropic::from_config(cfg)?)),
        "copilot" => Ok(Box::new(copilot::Copilot::from_config(cfg)?)),
        other => anyhow::bail!("unknown provider kind: {}", other),
    }
}
