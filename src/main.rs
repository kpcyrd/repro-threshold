mod app;
mod args;
mod attestation;
mod config;
mod errors;
mod event;
mod http;
mod inspect;
mod plumbing;
mod rebuilder;
mod transport;
mod ui;

use crate::app::App;
use crate::args::{Args, SubCommand};
use crate::config::Config;
use crate::errors::*;
use clap::Parser;
use env_logger::Env;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = "info";
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    match args.subcommand {
        None => {
            let config = Config::load().await?;

            let terminal = ratatui::init();
            let result = App::new(config).run(terminal).await;
            ratatui::restore();
            result
        }
        Some(SubCommand::Transport(transport)) => transport::run(transport).await,
        Some(SubCommand::Plumbing(plumbing)) => plumbing::run(plumbing).await,
    }
}
