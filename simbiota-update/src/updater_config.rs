use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct ServerConfig {
    pub host: String,
    pub architecture: String,
}

#[derive(Deserialize, Debug)]
pub struct UpdaterConfig {
    pub server: ServerConfig,
}

#[derive(Deserialize, Debug)]
pub struct FullConfig {
    pub updater: UpdaterConfig,
}
