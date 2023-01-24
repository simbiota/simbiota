use crate::updater_config::FullConfig;
use clap::Parser;
use log::{debug, error, LevelFilter};
use simbiota_clientlib::client_config::ClientConfig;
use std::path::PathBuf;

const DEFAULT_CONFIG_PATH: &str = "/etc/simbiota/client.yaml";

mod updater;
mod updater_config;

#[derive(Parser, Debug)]
pub(crate) struct UpdaterArgs {
    /// Specify a custom config file
    #[arg(short, long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,
}

fn main() {
    simple_logger::SimpleLogger::new()
        .env()
        .with_module_level("mio", LevelFilter::Warn)
        .with_module_level("reqwest", LevelFilter::Warn)
        .with_module_level("want", LevelFilter::Warn)
        .init()
        .unwrap();
    let args = UpdaterArgs::parse();

    let config_path = &args
        .config
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

    let client_config = ClientConfig::load_from(config_path, true);
    debug!("loading updater config from: {}", config_path.display());
    let updater_config: FullConfig =
        serde_yaml::from_reader(std::fs::File::open(config_path).unwrap()).expect("invalid config");
    println!("updater config: {:#?}", updater_config);

    let database_path = client_config.database.database_path;
    let update_result = updater::perform_update(
        database_path,
        updater_config.updater.server.host,
        updater_config.updater.server.architecture,
    );
    if update_result.is_err() {
        error!(
            "failed to update the database: {:?}",
            update_result.unwrap_err()
        );
    }
}
