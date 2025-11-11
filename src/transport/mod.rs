pub mod alpm;
pub mod apt;

use crate::args::Transport;
use crate::config::Config;
use crate::errors::*;

pub async fn run(transport: Transport) -> Result<()> {
    let config = Config::load().await?;

    match transport {
        Transport::Alpm { .. } => alpm::run(config).await,
        Transport::Apt => apt::run(config).await,
    }
}
