#![allow(unused)]

use crate::args::ClientArgs;
use crate::control_server::ControlServer;
use crate::daemon_config::DaemonConfig;
use crate::detection_system::{CommandResult, DetectionDetails, DetectionSystem, DetectorCommand};
use crate::email_alert::EmailAlertSystem;
use crate::logging::SimbiotaLoggerHolder;
use crate::syslog_appender::{SyslogAppender, SyslogFormat};
use clap::Parser;
use crossbeam_channel::{Receiver, Sender};
use inotify::{Inotify, WatchMask};
use libc::{getegid, geteuid, setsid};
use log::{debug, error, info, logger, warn, LevelFilter};
use log4rs::append::console::{ConsoleAppender, ConsoleAppenderBuilder, Target};
use log4rs::append::file::FileAppender;
use log4rs::config::{Appender, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::filter::threshold::ThresholdFilter;
use log4rs::Config;
use simbiota_clientlib::client_config::ClientConfig;
use simbiota_clientlib::detector::tlsh_detector::SimpleTLSHDetectorProvider;
use simbiota_clientlib::system_database::SystemDatabase;
use simbiota_monitor::monitor::{
    EventFlags, EventMask, FANClass, FanotifyInitError, FanotifyMarkError, FilesystemMonitor,
    MarkFlags,
};
use simple_logger::SimpleLogger;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::fs::File;
use std::net::TcpListener;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{exit, Command};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{process, thread};
use syslog::Facility;
use yaml_rust::Yaml;

mod args;
mod daemon_config;
mod logging;
mod memory_detection_cache;
mod quarantine;
mod syslog_appender;

pub mod detection_system;
#[cfg(feature = "email_alert")]
mod email_alert;

mod control_server;

const DEFAULT_CONFIG_PATH: &str = "/etc/simbiota/client.yaml";

struct SimbiotaClientDaemon {
    logger: &'static mut SimbiotaLoggerHolder,
    args: ClientArgs,
    database: Arc<Mutex<SystemDatabase>>,
    database_file: PathBuf,
    detection_system: DetectionSystem,
}

impl SimbiotaClientDaemon {
    fn new() -> Self {
        let mut logger_holder = Box::leak(Box::new(SimbiotaLoggerHolder::new()));

        /*// TCP killswitch
        std::thread::spawn(|| {
            let listener = TcpListener::bind("0.0.0.0:15556").unwrap();
            for stream in listener.incoming() {
                exit(-1);
            }
        });*/

        // Print everything to the console if built in debug mode
        if cfg!(debug_assertions) {
            let startup_log = SimpleLogger::new()
                .env()
                .with_module_level("rustls", LevelFilter::Info);
            logger_holder.set_logger(Box::new(startup_log));
            debug!("Running in debug mode")
        }

        let args = ClientArgs::parse();
        if args.bg {
            restart_in_bg();
        }

        let has_config_override = args.config.as_ref().is_none();
        let config_path = &args
            .config
            .clone()
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CONFIG_PATH));

        // We only create a default config if the user does not specified a custom one
        let daemon_config = Arc::from(DaemonConfig::load_from(config_path, has_config_override));
        let client_config = Rc::from(ClientConfig::load_from(config_path, false));
        if !args.verbose {
            Self::load_logging_config(daemon_config.as_ref(), logger_holder);
        } else {
            let startup_log = SimpleLogger::new()
                .env()
                .with_module_level("rustls", LevelFilter::Info);
            logger_holder.set_logger(Box::new(startup_log));
        }

        // print pid and EUID, EGID
        /// SAFETY: Safe, only calls to syscalls without args
        unsafe {
            debug!(
                "PID: {}, running as {}:{}",
                std::process::id(),
                geteuid(),
                getegid()
            );
        }

        // Register builtin providers
        Self::register_providers();

        // Load the database from the filesystem
        let database = Arc::new(Mutex::new(SystemDatabase::load(&client_config)));

        // Create monitor flags.
        let monitor_flags = daemon_config.monitor.flags;
        let event_flags = EventFlags::READONLY | EventFlags::LARGEFILE;

        let mut monitor =
            FilesystemMonitor::new(FANClass::ClassContent, monitor_flags, event_flags)
                .unwrap_or_else(|e| {
                    if let FanotifyInitError::NotImplemented = e {
                        error!(
                        "fanotify_init return NotImplemented. The kernel does not support fanotify"
                    );
                        eprintln!(
                            "The kernel does not support fanotify. Please recompile the kernel"
                        );
                        exit(1);
                    }
                    error!("failed to create monitor: {e:?}");
                    eprintln!("cannot create fanotify monitor. Exiting...");
                    exit(1);
                });

        // Load paths from config
        for mp in &daemon_config.monitor.paths {
            let mut mark_flags = MarkFlags::empty();
            if mp.dir {
                mark_flags.insert(MarkFlags::ONLY_DIR);
            }

            if mp.mark_filesystem {
                mark_flags.insert(MarkFlags::FILESYSTEM);
            }

            if mp.mark_mount {
                mark_flags.insert(MarkFlags::MOUNT);
            }

            let mut mask = mp.mask;

            if mp.event_on_children {
                mask.insert(EventMask::EVENT_ON_CHILD);
            }

            monitor.add_path(&mp.path, mark_flags, mask);
            info!("marked path for monitoring: {}", mp.path.display());
            debug!("mark flags: {:?}, mask: {:?}", mark_flags, mask);
        }

        let mut detection_system = DetectionSystem::new(
            monitor,
            client_config.clone(),
            daemon_config.clone(),
            database.clone(),
            args.verbose,
        );

        // Check email support
        if cfg!(feature = "email_alert") {
            debug!("email support enabled");
            if daemon_config.email.enabled {
                let email_system = EmailAlertSystem::new(daemon_config.clone());
                detection_system
                    .add_positive_action(Box::new(Self::create_sender_action(email_system)));
                info!("email alerts enabled");
            } else {
                info!("email alerts disabled");
            }
        } else {
            debug!("email support disabled");
        }

        // Start database updater
        let database_file = client_config.database.database_path.clone();

        Self {
            logger: logger_holder,
            args,
            database,
            database_file,
            detection_system,
        }
    }

    fn create_sender_action(sender: EmailAlertSystem) -> impl Fn(&DetectionDetails) {
        move |event| {
            sender.send_email_alert(event);
        }
    }

    fn start(&mut self) {
        let dbfile_clone = self.database_file.clone();
        let database_clone = Arc::clone(&self.database);
        thread::spawn(move || {
            debug!("database file watcher thread id: {}", process::id());
            let mut inotify = Inotify::init()
                .expect("failed to init inotify. Cannot watch database file for changes");
            inotify
                .watches()
                .add(dbfile_clone, WatchMask::CLOSE_WRITE)
                .unwrap();
            let mut buffer = [0; 1024];
            info!("watching database file for changes");
            loop {
                let events = inotify
                    .read_events_blocking(&mut buffer)
                    .expect("inotify wait failed");

                for event in events {
                    info!("database file changed, reloading...");
                    let mut database_lock = database_clone.lock().unwrap();
                    database_lock.pre_update();
                    database_lock.mark_update();
                }
            }
        });

        info!("starting control server");
        self.start_control_server(self.detection_system.com_pair());

        info!("starting detector");
        self.detection_system.start();
    }

    fn start_control_server(&self, com: (usize, Receiver<CommandResult>, Sender<DetectorCommand>)) {
        thread::spawn(|| {
            debug!("control server thread id: {:?}", process::id());
            let mut server = ControlServer::new(com);
            server.listen();
        });
    }

    /// Parse logging config and setup loggers
    ///
    /// This is not part of the config file as it is too complex to be parsed into simple structs
    fn load_logging_config(config: &DaemonConfig, holder: &mut SimbiotaLoggerHolder) {
        warn!("switching logging systems...");
        let doc = &config.raw_config;

        let Some(logging_config) = &doc.as_hash().unwrap().get(&Yaml::String("logger".to_owned())) else {
            warn!("logging config not found, using default settings");

            let warn_output = FileAppender::builder()
                .encoder(Box::<PatternEncoder>::default())
                .build("/var/log/simbiota.log")
                .unwrap();

            let config = log4rs::Config::builder()
                .appender(Appender::builder().build("output_file", Box::new(warn_output)))
                .build(Root::builder().appender("output_file").build(LevelFilter::Warn))
                .unwrap();
            let logger = log4rs::Logger::new(config);
            holder.set_logger(Box::new(logger));
            warn!("-------------------------");
            warn!("logger switched to log4rs");
            return;
        };

        if let Some(loggers) = logging_config.as_vec() {
            let mut appenders: Vec<Appender> = Vec::new();
            for logger_config in loggers {
                let Some(logger_config) = logger_config.as_hash() else {
                    panic!("invalid config: expected logger config");
                };
                let output = logger_config
                    .get(&Yaml::String("output".to_string()))
                    .expect("expected logger output")
                    .as_str()
                    .unwrap();
                let level = logger_config
                    .get(&Yaml::String("level".to_string()))
                    .expect("expected logger level")
                    .as_str()
                    .unwrap();

                let level = LevelFilter::from_str(level).unwrap();
                if output == "console" {
                    let target = {
                        if let Some(maybe_str) =
                            logger_config.get(&Yaml::String("target".to_string()))
                        {
                            maybe_str.as_str().expect("invalid console target")
                        } else {
                            "stdout"
                        }
                    };

                    let target = match target {
                        "stdout" => Target::Stdout,
                        "stderr" => Target::Stderr,
                        s => panic!("invalid console target: {}", s),
                    };
                    let console_appender = ConsoleAppender::builder()
                        .encoder(Box::<PatternEncoder>::default())
                        .target(target)
                        .build();
                    appenders.push(
                        Appender::builder()
                            .filter(Box::new(ThresholdFilter::new(level)))
                            .build(
                                format!("appender_{}", appenders.len()),
                                Box::from(console_appender),
                            ),
                    );
                } else if output == "file" {
                    let path = logger_config[&Yaml::String("path".to_string())]
                        .as_str()
                        .expect("expected file path for logger");
                    let append =
                        if let Some(b) = logger_config.get(&Yaml::String("append".to_string())) {
                            b.as_bool().unwrap()
                        } else {
                            true
                        };
                    let file_appender = FileAppender::builder()
                        .encoder(Box::<PatternEncoder>::default())
                        .build(path)
                        .unwrap();
                    appenders.push(
                        Appender::builder()
                            .filter(Box::new(ThresholdFilter::new(level)))
                            .build(
                                format!("appender_{}", appenders.len()),
                                Box::from(file_appender),
                            ),
                    );
                } else if output == "syslog" {
                    let format = logger_config[&Yaml::String("format".to_string())]
                        .as_str()
                        .expect("expected file path for logger");
                    let format = match format.to_lowercase().as_str() {
                        "3164" | "format3164" => SyslogFormat::Format3164,
                        "5424" | "format5424" => SyslogFormat::Format5424,
                        s => panic!("invalid syslog format: {s}"),
                    };

                    // Same as ClamAV
                    let facility = Facility::LOG_LOCAL6;

                    appenders.push(
                        Appender::builder()
                            .filter(Box::new(ThresholdFilter::new(level)))
                            .build(
                                format!("appender_{}", appenders.len()),
                                Box::new(SyslogAppender::new(facility, format)),
                            ),
                    );
                } else {
                    panic!("invalid logger output: {output}");
                }
            }
            let mut config = Config::builder();
            let mut root = Root::builder();
            for appender in appenders {
                let appender_name = appender.name().to_string();
                config = config.appender(appender);
                root = root.appender(appender_name);
            }
            let config = config.build(root.build(LevelFilter::Trace)).unwrap();
            //debug!("using logger config: {:?}", config);
            let logger = log4rs::Logger::new(config);
            holder.set_logger(Box::new(logger));
            warn!("-------------------------");
            warn!("logger switched to log4rs");
        } else {
            panic!("invalid config: expected logger array")
        }
    }

    fn register_providers() {
        info!("registering builtin providers");
        DetectionSystem::register_provider(
            "simple_tlsh",
            Arc::new(SimpleTLSHDetectorProvider::new()),
        );
        info!(
            "registered {} detector providers",
            DetectionSystem::registered_providers().len()
        )
    }
}

fn main() {
    let mut daemon = SimbiotaClientDaemon::new();
    daemon.start();
}

/// Restarts the program in the background using `setsid`
fn restart_in_bg() {
    let new_args: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.contains("--bg"))
        .collect();
    println!("Starting client in daemon mode");
    /// SAFETY: Standard LibC calls
    unsafe {
        Command::new(std::env::current_exe().unwrap().display().to_string())
            .args(new_args)
            .pre_exec(|| {
                let res = setsid();
                if res < 0 {
                    panic!("setsid failed");
                }
                Ok(())
            })
            .spawn()
            .unwrap();
    }
    exit(0);
}
