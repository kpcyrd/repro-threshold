use clap::{ArgAction, CommandFactory, Parser};
use clap_complete::Shell;
use reqwest::Url;
use std::io::stdout;
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Args {
    /// Increase logging output (can be used multiple times)
    #[arg(short, long, global = true, action(ArgAction::Count))]
    pub verbose: u8,
    #[clap(subcommand)]
    pub subcommand: Option<SubCommand>,
}

#[derive(Debug, Parser)]
pub enum SubCommand {
    #[clap(subcommand)]
    Transport(Transport),
    #[clap(subcommand)]
    Plumbing(Plumbing),
}

/// Integrations for package managers
#[derive(Debug, Parser)]
pub enum Transport {
    /// Integrations for Pacman's XferCommand= option
    Alpm {
        /// The output file path
        #[arg(short = 'O', long)]
        output: PathBuf,
        /// The package to download
        url: Url,
        #[command(flatten)]
        options: TransportOptions,
    },
    /// Integrations for APT's transport methods
    Apt,
}

#[derive(Debug, Parser)]
pub struct TransportOptions {
    /*
    /// Example: socks5://127.0.0.1:9050
    #[arg(long)]
    pub proxy: Option<Proxy>,
    /// Only use the proxy for transparency signatures, not the pkg
    #[arg(long)]
    pub bypass_proxy_for_pkgs: bool,
    */
    /// Use these rebuilders instead of the configured ones
    #[arg(long = "rebuilder")]
    pub rebuilders: Vec<Url>,
    /// Number of required confirms to accept a package as reproduced
    #[arg(long)]
    pub required_confirms: Option<usize>,
    /// Blindly allow these packages, even if nobody could reproduce the binary
    #[arg(long)]
    pub blindly_allow: Vec<String>,
}

/// Low-level commands and utilities
#[derive(Debug, Parser)]
pub enum Plumbing {
    /// Fetch a curated list of well-known rebuilders
    FetchRebuilderdCommunity,
    /// Add a new rebuilder as trusted
    AddRebuilder {
        /// The rebuilder URL
        url: Url,
        /// Set a human-friendly name for the rebuilder (defaults to the URL domain)
        #[arg(long = "name")]
        name: Option<String>,
    },
    RemoveRebuilder {
        /// The rebuilder URL
        url: Url,
    },
    /// List configured rebuilders
    ListRebuilders {
        /// Show all known rebuilders, not just active/trusted ones
        #[arg(short = 'a', long = "all")]
        all: bool,
    },
    /// Authenticate a package through rebuilder attestations
    Verify {
        #[arg(short = 'S', long = "signing-key")]
        signing_keys: Vec<PathBuf>,
        #[arg(short = 'A', long = "attestation")]
        attestations: Vec<PathBuf>,
        #[arg(short = 'R', long = "rebuilder")]
        rebuilders: Vec<Url>,
        #[arg(short = 't', long = "threshold")]
        threshold: usize,
        /// The file to authenticate
        file: PathBuf,
    },
    /// Parse metadata from a .deb file
    InspectDeb {
        /// The .deb file to inspect
        file: PathBuf,
    },
    Completions(Completions),
}

/// Generate shell completions
#[derive(Debug, Parser)]
pub struct Completions {
    pub shell: Shell,
}

impl Completions {
    pub fn generate(&self) {
        clap_complete::generate(
            self.shell,
            &mut Args::command(),
            env!("CARGO_PKG_NAME"),
            &mut stdout(),
        );
    }
}
