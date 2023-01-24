use crate::quarantine::Quarantine;
use fanotify_monitor::monitor::{EventMask, MonitorFlags};
use linked_hash_map::LinkedHashMap;
use log::{debug, info, warn};
use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::exit;
use yaml_rust::{Yaml, YamlEmitter, YamlLoader};

#[derive(Debug)]
pub struct DetectorConfig {
    pub(crate) class: String,
    pub(crate) config: HashMap<String, Box<dyn Any>>,
}

#[derive(Debug)]
pub struct MonitoredPath {
    pub(crate) path: PathBuf,
    pub(crate) dir: bool,
    pub(crate) event_on_children: bool,
    pub(crate) mark_filesystem: bool,
    pub(crate) mark_mount: bool,
    pub(crate) mask: EventMask,
}

#[derive(Debug)]
pub(crate) enum SmtpConnectionSecurity {
    None,
    Ssl,
    Starttls,
}

#[derive(Debug)]
pub(crate) struct SmtpConfig {
    pub(crate) server: String,
    pub(crate) port: u16,
    pub(crate) username: String,
    pub(crate) password: Option<String>,
    pub(crate) security: SmtpConnectionSecurity,
}

#[derive(Debug)]
pub(crate) struct EmailConfig {
    pub(crate) enabled: bool,
    pub(crate) smtp_config: Option<SmtpConfig>,
    pub(crate) recipients: Vec<String>,
}

#[derive(Debug)]
pub struct MonitorConfig {
    pub(crate) flags: MonitorFlags,
    pub(crate) paths: Vec<MonitoredPath>,
}

#[derive(Debug)]
pub struct CacheConfig {
    pub(crate) disable_cache: bool,
}

#[derive(Debug)]
pub struct DatabaseConfig {
    pub(crate) database_path: PathBuf,
    pub(crate) low_memory: bool,
}

#[derive(Debug)]
pub(crate) struct QuarantineConfig {
    pub(crate) enabled: bool,
    pub(crate) path: PathBuf,
}

#[derive(Debug)]
pub struct DaemonConfig {
    pub(crate) monitor: MonitorConfig,
    pub(crate) email: EmailConfig,
    pub(crate) cache: Option<CacheConfig>,
    pub(crate) raw_config: Yaml,
    pub(crate) quarantine: QuarantineConfig,
}

const DEFAULT_MONITOR_FLAGS: MonitorFlags = MonitorFlags::empty()
    .union(MonitorFlags::CLOEXEC)
    .union(MonitorFlags::UNLIMITED_MARKS)
    .union(MonitorFlags::UNLIMITED_QUEUE);

impl DaemonConfig {
    pub(crate) fn load_from(path: &Path, use_default: bool) -> Self {
        debug!("loading config from {}", path.display());

        if !path.exists() {
            if use_default {
                let config = DaemonConfig::default();
                warn!("config-file does not exists. Using default config.");
                return config;
            } else {
                eprintln!("The specified config file does not exists! Bailing out... ");
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

        // TODO: Normal error handling instead of expect
        let mut mpaths = Vec::new();
        let monitor_config = doc["monitor"]
            .as_hash()
            .expect("invalid monitor config, expected dict");

        let flags = if monitor_config.contains_key(&Yaml::String("flags".to_string())) {
            let flags = monitor_config[&Yaml::String("flags".to_string())]
                .as_vec()
                .expect("invalid flags value");
            MonitorFlags::parse(flags.iter().map(|f| f.as_str().unwrap()).collect())
                .expect("failed to parse flags")
        } else {
            DEFAULT_MONITOR_FLAGS
        };

        let monitored_paths = monitor_config[&Yaml::String("paths".to_owned())]
            .as_vec()
            .expect("missing monitored paths array");

        for monitored_path in monitored_paths {
            let masks = monitored_path["mask"]
                .as_vec()
                .expect("monitored_path mask expected");
            let mut mpath_masks = HashSet::new();
            for mask in masks {
                mpath_masks.insert(mask.as_str().expect("mask string expected"));
            }

            let mpath = MonitoredPath {
                path: PathBuf::from(
                    monitored_path["path"]
                        .as_str()
                        .expect("expected path in monitored_path"),
                ),
                dir: monitored_path["dir"].as_bool().unwrap_or(false),
                mark_mount: monitored_path["mount"].as_bool().unwrap_or(false),
                mark_filesystem: monitored_path["filesystem"].as_bool().unwrap_or(false),
                event_on_children: monitored_path["event_on_children"]
                    .as_bool()
                    .unwrap_or(false),
                mask: EventMask::parse(mpath_masks.iter().copied().collect())
                    .expect("failed to parse mark mask"),
            };

            mpaths.push(mpath);
        }

        // Load email config
        let email_cfg = doc["email"].as_hash();
        let email_config = if let Some(email_cfg_data) = email_cfg {
            let enabled = email_cfg_data[&Yaml::from_str("enabled")]
                .as_bool()
                .unwrap_or(false);
            let email_config = if enabled {
                let smtp_config = email_cfg_data[&Yaml::from_str("smtp")]
                    .as_hash()
                    .expect("you must provide an smtp server if you want email alerts");
                let smtp_server = smtp_config[&Yaml::from_str("server")]
                    .as_str()
                    .expect("smtp server missing");
                let port = smtp_config[&Yaml::from_str("port")].as_i64().unwrap_or(587);

                let username = smtp_config[&Yaml::from_str("username")]
                    .as_str()
                    .expect("smtp user missing");
                let maybe_password = smtp_config[&Yaml::from_str("password")].as_str();
                let security = smtp_config[&Yaml::from_str("security")]
                    .as_str()
                    .unwrap_or("none");

                let recipients = email_cfg_data[&Yaml::from_str("recipients")]
                    .as_vec()
                    .expect("email recipients missing")
                    .iter()
                    .map(|y| y.as_str().unwrap().to_string())
                    .collect();

                EmailConfig {
                    enabled: true,
                    smtp_config: Some(SmtpConfig {
                        server: smtp_server.to_string(),
                        port: port as u16,
                        username: username.to_string(),
                        password: maybe_password.map(|s| s.to_string()),
                        security: match security.to_ascii_lowercase().as_str() {
                            "none" => SmtpConnectionSecurity::None,
                            "ssl" => SmtpConnectionSecurity::Ssl,
                            "tls" | "starttls" => SmtpConnectionSecurity::Starttls,
                            _ => panic!("invalid smtp connection security"),
                        },
                    }),
                    recipients,
                }
            } else {
                EmailConfig {
                    enabled: false,
                    smtp_config: None,
                    recipients: Vec::new(),
                }
            };
            email_config
        } else {
            info!("email config not found. alerts disabled");
            EmailConfig {
                enabled: false,
                smtp_config: None,
                recipients: Vec::new(),
            }
        };

        let detector_cfg = doc["detector"].as_hash().expect("detector config expected");
        let class = detector_cfg[&Yaml::String("class".to_string())]
            .as_str()
            .expect("detector class expected");
        let mut config = HashMap::new();
        if let Some(detector_configs_y) = detector_cfg.get(&Yaml::String("config".to_string())) {
            if let Some(detector_configs) = detector_configs_y.as_hash() {
                for (key, val) in detector_configs.iter() {
                    let key_string = key.as_str().unwrap().to_string();
                    let value = DaemonConfig::yaml_to_any(val);
                    config.insert(key_string, value);
                }
            }
        }

        let cache_disabled = {
            if let Some(cache_cfg) = doc["cache"].as_hash() {
                if let Some(disabled_v) = cache_cfg[&Yaml::String("disable".to_string())].as_bool()
                {
                    disabled_v
                } else {
                    false
                }
            } else {
                false
            }
        };

        // Load database config
        let database_cfg = doc["database"].as_hash().expect("database config expected");
        let path = database_cfg[&Yaml::String("database_file".to_owned())]
            .as_str()
            .expect("database file config missing");

        let database_config = DatabaseConfig {
            database_path: PathBuf::from(path),
            low_memory: false,
        };

        if cache_disabled {
            debug!("detection cache is disabled in config");
        }

        let quarantine_cfg = doc["quarantine"].as_hash();
        let quarantine_config = if let Some(quarantine_cfg) = quarantine_cfg {
            let enabled = quarantine_cfg[&Yaml::String("enabled".to_string())]
                .as_bool()
                .unwrap_or(false);
            let path = if enabled {
                PathBuf::from(
                    quarantine_cfg[&Yaml::String("path".to_string())]
                        .as_str()
                        .expect("quarantine path expected"),
                )
            } else {
                Default::default()
            };
            QuarantineConfig { enabled, path }
        } else {
            QuarantineConfig {
                enabled: false,
                path: Default::default(),
            }
        };

        Self {
            monitor: MonitorConfig {
                flags,
                paths: mpaths,
            },
            email: email_config,
            cache: Some(CacheConfig {
                disable_cache: cache_disabled,
            }),
            quarantine: quarantine_config,
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
                    vec.push(DaemonConfig::yaml_to_any(element));
                }
                Box::new(vec)
            }
            Yaml::Hash(v) => {
                let mut map = HashMap::new();
                for (key, val) in v {
                    let key = key.as_str().unwrap().to_string();
                    let val = DaemonConfig::yaml_to_any(val);
                    map.insert(key, val);
                }
                Box::new(map)
            }
            _ => panic!("invalid config"),
        };
        value
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            monitor: MonitorConfig {
                flags: DEFAULT_MONITOR_FLAGS,
                paths: vec![MonitoredPath {
                    path: PathBuf::from("/usr/bin"),
                    dir: true,
                    mark_mount: false,
                    mark_filesystem: false,
                    event_on_children: true,
                    mask: EventMask::OPEN_EXEC_PERM,
                }],
            },
            email: EmailConfig {
                enabled: false,
                smtp_config: None,
                recipients: Vec::new(),
            },
            quarantine: QuarantineConfig {
                enabled: true,
                path: PathBuf::from("/var/lib/simbiota/quarantine"),
            },
            cache: None,
            raw_config: Yaml::Null,
        }
    }
}
