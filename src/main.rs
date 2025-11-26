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
mod signing;
mod transport;
mod ui;
mod withhold;

use crate::app::App;
use crate::args::{Args, SubCommand};
use crate::config::Config;
use crate::errors::*;
use clap::Parser;
use env_logger::Env;
use std::env;

fn is_apt_transport_multicall() -> bool {
    let Some(bin) = env::args_os().next() else {
        return false;
    };
    let Ok(bin) = bin.into_string() else {
        return false;
    };
    let Some(bin) = bin.rsplit('/').next() else {
        return false;
    };
    bin.starts_with("reproduced+")
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = match args.verbose {
        0 => "repro_threshold=info",
        1 => "info,repro_threshold=debug",
        2 => "debug",
        3 => "debug,repro_threshold=trace",
        _ => "trace",
    };
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    match args.subcommand {
        None if is_apt_transport_multicall() => transport::run(args::Transport::Apt).await,
        None => {
            let config = Config::load_writable().await?;

            let terminal = ratatui::init();
            let result = App::new(config).run(terminal).await;
            ratatui::restore();
            result
        }
        Some(SubCommand::Transport(transport)) => transport::run(transport).await,
        Some(SubCommand::Plumbing(plumbing)) => plumbing::run(plumbing).await,
    }
}
