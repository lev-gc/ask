use clap::{Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum EnvLevel {
    Min,
    Full,
    Tools,
}

#[derive(Parser, Debug)]
#[command(name = "ask", version, about = "AI-powered Linux command assistant")]
pub struct Cli {
    /// Override provider name (e.g. openai, kimi, deepseek, anthropic, copilot)
    #[arg(short, long)]
    pub provider: Option<String>,

    /// Override model name
    #[arg(short, long)]
    pub model: Option<String>,

    /// Environment probe level
    #[arg(short, long, value_enum, default_value_t = EnvLevel::Min)]
    pub env: EnvLevel,

    /// Disable streaming output
    #[arg(long)]
    pub no_stream: bool,

    /// Path to config file
    #[arg(long)]
    pub config: Option<PathBuf>,

    /// The question / intent. All trailing args joined with spaces.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub question: Vec<String>,
}

impl Cli {
    pub fn question_text(&self) -> String {
        self.question.join(" ")
    }
}
