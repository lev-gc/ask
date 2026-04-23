mod cli;
mod config;
mod env_probe;
mod interact;
mod prompt;
mod provider;
mod render;
mod stream;

use anyhow::Result;
use clap::Parser;
use futures_util::StreamExt;
use std::io::{IsTerminal, Read};

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = cli::Cli::parse();
    if cli.question.is_empty() && std::io::stdin().is_terminal() {
        eprintln!("usage: ask [OPTIONS] <question>\n\ntry: ask check disk usage");
        std::process::exit(2);
    }

    let question = cli.question_text();

    let stdin_text = if !std::io::stdin().is_terminal() {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).ok();
        if s.trim().is_empty() { None } else { Some(s) }
    } else {
        None
    };

    let env_info = env_probe::probe(cli.env);
    let env_block = env_probe::format_for_prompt(&env_info);
    let user_msg = prompt::build_user_message(&env_block, stdin_text.as_deref(), &question);

    let (cfg, _path) = match config::load(cli.config.as_deref())? {
        config::Loaded::Existing(cfg, path) => (cfg, path),
        config::Loaded::TemplateCreated(path) => {
            eprintln!("Welcome to ask! Looks like this is your first run.");
            eprintln!();
            eprintln!("A starter config has been created at:");
            eprintln!("    {}", path.display());
            eprintln!();
            eprintln!("Open it in your editor and either:");
            eprintln!("  - set `api_key = \"...\"` for your chosen provider, or");
            eprintln!("  - export the environment variable named in `api_key_env`");
            eprintln!("    (e.g. `export OPENAI_API_KEY=sk-...`).");
            eprintln!();
            eprintln!("Then run `ask` again.");
            return Ok(());
        }
    };
    let (_name, provider_cfg) = config::resolve_provider(&cfg, cli.provider.as_deref())?;
    let model = cli
        .model
        .clone()
        .unwrap_or_else(|| provider_cfg.model.clone());

    let provider = provider::build(provider_cfg)?;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let first_cmd = rt.block_on(async move {
        let req = provider::ChatRequest {
            system: prompt::SYSTEM,
            user: &user_msg,
            model: &model,
            stream: !cli.no_stream,
        };
        let mut stream = provider.chat(req).await?;
        let mut renderer = render::Renderer::new();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(s) => renderer.feed(&s),
                Err(e) => {
                    renderer.finish();
                    return Err::<Option<String>, anyhow::Error>(e);
                }
            }
        }
        renderer.finish();
        Ok(renderer.first_command().map(|s| s.to_string()))
    })?;

    interact::copy_first_command(first_cmd.as_deref());
    Ok(())
}
