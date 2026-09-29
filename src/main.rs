//! Aufseher: a Telegram bot that bans users whose names or messages match patterns from a YAML
//! file.
//!
//! [`rules`] compiles the pattern file, [`detection`] decides which senders a message reveals as
//! spammers, [`handlers`] is the dispatcher endpoint that acts on the verdict, [`actions`] holds
//! the Telegram operations it uses, and [`reload`] swaps in the rules when the pattern file
//! changes.

// A test panics on failure by design, so the `[lints.clippy]` panic lints in `Cargo.toml` are
// relaxed for test builds alone.
#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

mod actions;
mod detection;
mod handlers;
mod reload;
mod rules;

use std::{path::PathBuf, process::ExitCode, sync::Arc};

use anyhow::{Context, Result};
use clap::Parser;
use teloxide::{error_handlers::LoggingErrorHandler, prelude::*, update_listeners};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    /// Telegram bot API token
    #[arg(short, long, env = "TELEGRAM_BOT_TOKEN", hide_env_values = true)]
    token: String,

    /// Path to the pattern file
    #[arg(short, long, default_value = "/etc/aufseher.yaml")]
    config_file: PathBuf,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    match run(Args::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            error!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

/// Loads and watches the pattern file, then handles updates until interrupted.
async fn run(args: Args) -> Result<()> {
    info!("Aufseher {} initializing", env!("CARGO_PKG_VERSION"));
    let rules = reload::spawn(args.config_file)?;
    let bot = Bot::new(args.token);

    let handler = dptree::entry()
        .branch(Update::filter_message().endpoint(handlers::handle_message))
        .branch(Update::filter_edited_message().endpoint(handlers::handle_message));
    let mut dispatcher = Dispatcher::builder(bot.clone(), handler)
        .dependencies(dptree::deps![rules])
        .error_handler(Arc::new(|error: anyhow::Error| async move {
            error!("{error:#}");
        }))
        .enable_ctrlc_handler()
        .build();

    info!("Initialization complete, starting to handle updates");
    let listener = update_listeners::polling_default(bot).await;
    dispatcher
        .try_dispatch_with_listener(
            listener,
            LoggingErrorHandler::with_custom_text("An error from the update listener"),
        )
        .await
        .context("failed to connect to the Telegram Bot API")
}
