use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub subsys: Subsys,
}

#[derive(Subcommand)]
pub enum Subsys {
    /// Manual scan operations
    /*Scan {
        #[command(subcommand)]
        command: ScanCommand,
    },*/
    /// Quarantine operations
    Quarantine {
        #[command(subcommand)]
        command: QuarantineCommand,
    },
}

#[derive(Subcommand)]
pub enum ScanCommand {
    /// Start a manual scan of a file or directory
    Start {
        /// Path of the file or directory to scan
        path: PathBuf,
        /// Recursive scan of directory
        #[arg(short, long)]
        recursive: bool,
    },
    /// List running scans
    List,
    /// Cancel a running scan
    Cancel { id: String },
}

#[derive(Subcommand)]
pub enum QuarantineCommand {
    /// List quarantined files
    List,
    /// Restore a file from quarantine
    Restore { id_or_path: String },
    /// Permanently delete a file from quarantine
    Delete { id_or_path: String },
}
