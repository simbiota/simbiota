use log::{info, warn};
use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::exit;
use yaml_rust::{Yaml, YamlLoader};

#[derive(Debug)]
pub struct DetectorConfig {
    pub class: String,
    pub config: HashMap<String, Box<dyn Any>>,
}

#[derive(Debug)]
pub struct DatabaseConfig {
    pub database_path: PathBuf,
    pub(crate) low_memory: bool,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ClientConfig {
    pub detector: DetectorConfig,
    pub database: DatabaseConfig,
    pub(crate) raw_config: Yaml,
}

impl ClientConfig {
    pub fn load_from(path: &Path, use_default: bool) -> Self {
        info!("loading config from {}", path.display());

        if !path.exists() {
            if use_default {
                warn!("config file not found at the default location! Using default config");
                let config = ClientConfig::default();
                return config;
            } else {
                eprintln!("The specified config file does not exists! Bailing out...");
                exit(1);
            }
        }

        let config_content = {
            let maybe_contents = std::fs::read_to_string(path);
            let Ok(content) = maybe_contents else {
                eprintln!("The specified config file does not exists! Bailing out...");
                exit(1);
            };
            content
        };

        let maybe_doc = YamlLoader::load_from_str(config_content.as_str());
        let Ok(doc) = maybe_doc else {
            warn!("failed to parse YAML config: {}", maybe_doc.unwrap_err().to_string());
            eprintln!("The specified config is not valid! Bailing out...");
            exit(1);
        };
        Self::from_yaml(doc)
    }

    fn from_yaml(mut yaml: Vec<Yaml>) -> Self {
        let doc = yaml.remove(0);

        let detector_cfg = doc["detector"].as_hash().expect("detector config expected");
        let class = detector_cfg[&Yaml::String("class".to_string())]
            .as_str()
            .expect("detector class expected");
        let mut config = HashMap::new();
        if let Some(detector_configs_y) = detector_cfg.get(&Yaml::String("config".to_string())) {
            if let Some(detector_configs) = detector_configs_y.as_hash() {
                for (key, val) in detector_configs.iter() {
                    let key_string = key.as_str().unwrap().to_string();
                    let value = ClientConfig::yaml_to_any(val);
                    config.insert(key_string, value);
                }
            }
        }

        // Load database config
        let database_cfg = doc["database"].as_hash().expect("database config expected");
        let path = database_cfg[&Yaml::String("database_file".to_owned())]
            .as_str()
            .expect("database file config missing");

        let database_config = DatabaseConfig {
            database_path: PathBuf::from(path),
            low_memory: false,
        };

        Self {
            detector: DetectorConfig {
                class: class.to_string(),
                config,
            },
            database: database_config,
            raw_config: doc,
        }
    }

    fn yaml_to_any(yaml: &Yaml) -> Box<dyn Any> {
        let value: Box<dyn Any> = match yaml {
            Yaml::Real(v) => Box::new(v.clone()),
            Yaml::Integer(v) => Box::new(*v),
            Yaml::String(v) => Box::new(v.clone()),
            Yaml::Boolean(v) => Box::new(*v),
            Yaml::Array(v) => {
                let mut vec = Vec::new();
                for element in v {
                    vec.push(ClientConfig::yaml_to_any(element));
                }
                Box::new(vec)
            }
            Yaml::Hash(v) => {
                let mut map = HashMap::new();
                for (key, val) in v {
                    let key = key.as_str().unwrap().to_string();
                    let val = ClientConfig::yaml_to_any(val);
                    map.insert(key, val);
                }
                Box::new(map)
            }
            _ => panic!("invalid config"),
        };
        value
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            detector: DetectorConfig {
                class: "dummy".to_string(),
                config: Default::default(),
            },
            database: DatabaseConfig {
                database_path: PathBuf::from("/var/lib/simbiota/database.sdb"),
                low_memory: false,
            },
            raw_config: Yaml::Null,
        }
    }
}
