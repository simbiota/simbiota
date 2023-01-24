use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub(crate) struct ClientArgs {
    /// Specify a custom config file
    #[arg(short, long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,

    /// Run in daemon mode
    #[arg(long)]
    pub(crate) bg: bool,

    /// Verbose output
    #[arg(short, long)]
    pub(crate) verbose: bool,
}
