mod app;
mod args;
mod attestation;
mod config;
mod errors;
mod event;
mod http;
mod plumbing;
mod rebuilder;
mod ui;

use crate::app::App;
use crate::args::{Args, SubCommand, Transport};
use crate::config::Config;
use crate::errors::*;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.subcommand {
        None => {
            let config = Config::load().await?;

            let terminal = ratatui::init();
            let result = App::new(config).run(terminal).await;
            ratatui::restore();
            result
        }
        Some(SubCommand::Transport(transport)) => match transport {
            Transport::Alpm { .. } => todo!("alpm"),
            Transport::Apt => todo!("apt"),
        },
        Some(SubCommand::Plumbing(plumbing)) => plumbing::run(&plumbing).await,
    }
}
