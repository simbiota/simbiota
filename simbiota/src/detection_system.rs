use std::cell::RefCell;
use std::collections::HashMap;

use chrono::{Local, Utc};
use std::fs::File;
use std::num::ParseIntError;

use crossbeam_channel::{Receiver, RecvError, Sender};
use log::{debug, error, info, log, trace, warn};
use once_cell::sync::Lazy;
use simbiota_database::Database;
use std::ops::Deref;
use std::os::fd::FromRawFd;
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;
use std::{process, thread};

use simbiota_clientlib::api::cache::{DetectionCache, NoopCache};
use simbiota_clientlib::api::detector::DetectionResult::Match;
use simbiota_clientlib::api::detector::{DetectionResult, Detector};
use simbiota_clientlib::client_config::ClientConfig;
use simbiota_clientlib::detector::DetectorProvider;
use simbiota_clientlib::system_database::SystemDatabase;
use simbiota_monitor::monitor::{fanotify_event_metadata, FilesystemMonitor};
use simbiota_monitor::FanotifyEventResponse;
use simbiota_monitor::FanotifyEventResponse::{Allow, Deny};

use crate::daemon_config::DaemonConfig;
use crate::memory_detection_cache::MemoryDetectionCache;
use crate::quarantine::{Quarantine, QuarantineEntryInfo};

pub type DetectionSystemAction = Arc<Mutex<Box<dyn Fn(&DetectionDetails) + Send>>>;
pub struct DetectionSystem {
    positive_detection_action: Vec<DetectionSystemAction>,
    monitor: Arc<FilesystemMonitor>,
    detector: RefCell<Box<dyn Detector>>,
    cache: RefCell<Box<dyn DetectionCache<fanotify_event_metadata>>>,
    quarantine: Option<Arc<Mutex<Quarantine>>>,
    channels: RefCell<HashMap<usize, Sender<CommandResult>>>,
    detector_rx: Receiver<DetectorCommand>,
    client_tx: Sender<DetectorCommand>,
    next_detector_id: RefCell<usize>,
    daemon_pid: u32,
}

pub struct DetectionDetails {
    pub path: String,
    pub time: chrono::DateTime<Utc>,
}

static REGISTERED_PROVIDERS: Lazy<Mutex<HashMap<String, Arc<dyn DetectorProvider + Send + Sync>>>> =
    Lazy::new(|| {
        let m = HashMap::new();

        Mutex::new(m)
    });

pub struct DetectorCommand {
    pub id: usize,
    pub command: Action,
}
pub enum Action {
    FanotifyEvent(fanotify_event_metadata),
    FanotifyEventWithResponse(fanotify_event_metadata),
    QueryQuarantine,
    RestoreQuarantineEntry(String),
    DeleteQuarantineEntry(String),
}
pub enum CommandResult {
    FanotifyResponse(FanotifyEventResponse),
    QuarantineEntries(Vec<QuarantineEntryInfo>),
    QuarantineAction(bool),
}

impl DetectionSystem {
    pub fn register_provider(name: &str, provider: Arc<dyn DetectorProvider + Send + Sync>) {
        REGISTERED_PROVIDERS
            .lock()
            .unwrap()
            .insert(name.to_string(), provider);
        debug!("registered detector: {name}")
    }

    pub fn registered_providers() -> HashMap<String, Arc<dyn DetectorProvider + Send + Sync>> {
        REGISTERED_PROVIDERS.lock().unwrap().deref().clone()
    }

    pub fn new(
        monitor: FilesystemMonitor,
        client_config: Rc<ClientConfig>,
        daemon_config: Arc<DaemonConfig>,
        database: Arc<Mutex<SystemDatabase>>,
        verbose_log: bool,
    ) -> Self {
        let detector_config = &client_config.detector;
        let class = &detector_config.class;

        let map = REGISTERED_PROVIDERS.lock().unwrap();
        let provider = map.get(class).expect("invalid detector class");
        let detector = provider.get_detector(&detector_config.config, database);
        info!("using detector: {}", class);

        let detector = RefCell::from(detector);

        let cache: RefCell<Box<dyn DetectionCache<fanotify_event_metadata>>> =
            if is_cache_disabled(daemon_config.as_ref()) {
                RefCell::from(Box::new(NoopCache {}))
            } else {
                RefCell::from(Box::new(MemoryDetectionCache::new()))
            };

        // Quarantine setup
        let quarantine = if daemon_config.quarantine.enabled {
            let quarantine = Quarantine::new(daemon_config);
            Some(Arc::new(Mutex::from(quarantine)))
        } else {
            None
        };
        let (client_tx, detector_rx) = crossbeam_channel::unbounded();
        Self {
            positive_detection_action: Vec::new(),
            monitor: Arc::from(monitor),
            detector,
            cache,
            quarantine,
            channels: RefCell::from(HashMap::new()),
            client_tx,
            detector_rx,
            next_detector_id: RefCell::new(0),
            daemon_pid: std::process::id(),
        }
    }

    pub fn com_pair(&self) -> (usize, Receiver<CommandResult>, Sender<DetectorCommand>) {
        let (caller_tx, detector_rx) = crossbeam_channel::unbounded();

        let mut id = self.next_detector_id.borrow_mut();
        self.channels.borrow_mut().insert(*id, caller_tx);
        *id += 1;

        (*id - 1, detector_rx, self.client_tx.clone())
    }

    pub fn start(&self) -> ! {
        // create monitor channel

        let (monitor_id, client_rx, client_tx) = self.com_pair();
        let monitor = self.monitor.clone();

        // start fanotify responder thread
        thread::spawn(move || {
            debug!("monitor thread id: {:?}", process::id());
            let client = client_tx.clone();
            let client2 = client_tx.clone();
            monitor.start(
                Arc::new(move |e| {
                    client2
                        .send(DetectorCommand {
                            id: monitor_id,
                            command: Action::FanotifyEvent(*e),
                        })
                        .unwrap();
                }),
                Arc::new(move |e: &fanotify_event_metadata| {
                    //eprintln!("sending detector command");
                    client
                        .send(DetectorCommand {
                            id: monitor_id,
                            command: Action::FanotifyEventWithResponse(*e),
                        })
                        .unwrap();
                    //eprintln!("waiting for result");
                    match client_rx.recv() {
                        Ok(response) => {
                            if let CommandResult::FanotifyResponse(response) = response {
                                //eprintln!("got result");
                                response
                            } else {
                                panic!("invalid response from detector")
                            }
                        }

                        Err(err) => {
                            panic!("error receiving response from detector: {}", err);
                        }
                    }
                }),
            );
        });

        // receive commands and process them
        loop {
            let req: Result<DetectorCommand, RecvError> = self.detector_rx.recv();
            match req {
                Ok(cmd) => match cmd.command {
                    Action::FanotifyEvent(e) => {
                        self.detector_callback(&e);
                    }
                    Action::FanotifyEventWithResponse(e) => {
                        let response = self.detector_callback(&e);
                        let _ = self
                            .channels
                            .borrow()
                            .get(&cmd.id)
                            .unwrap()
                            .send(CommandResult::FanotifyResponse(response));
                    }
                    Action::QueryQuarantine => match &self.quarantine {
                        Some(quarantine) => {
                            let quarantine = quarantine.lock().unwrap();
                            let entries = quarantine.get_entries();

                            let _ = self
                                .channels
                                .borrow()
                                .get(&cmd.id)
                                .unwrap()
                                .send(CommandResult::QuarantineEntries(entries));
                        }
                        None => {
                            let _ = self
                                .channels
                                .borrow()
                                .get(&cmd.id)
                                .unwrap()
                                .send(CommandResult::QuarantineEntries(vec![]));
                        }
                    },
                    Action::RestoreQuarantineEntry(e) => match &self.quarantine {
                        Some(quarantine) => {
                            let mut quarantine = quarantine.lock().unwrap();

                            let maybe_id = e.parse::<usize>();
                            let entry = match maybe_id {
                                Ok(id) => quarantine.get_entry_by_id(id),
                                Err(_) => quarantine.get_entry_by_name(&e),
                            };

                            if let Some(entry) = entry {
                                quarantine.restore_entry(entry);
                                let _ = self
                                    .channels
                                    .borrow()
                                    .get(&cmd.id)
                                    .unwrap()
                                    .send(CommandResult::QuarantineAction(true));
                            } else {
                                let _ = self
                                    .channels
                                    .borrow()
                                    .get(&cmd.id)
                                    .unwrap()
                                    .send(CommandResult::QuarantineAction(false));
                            }
                        }
                        None => {
                            let _ = self
                                .channels
                                .borrow()
                                .get(&cmd.id)
                                .unwrap()
                                .send(CommandResult::QuarantineAction(false));
                        }
                    },
                    Action::DeleteQuarantineEntry(e) => match &self.quarantine {
                        Some(quarantine) => {
                            let mut quarantine = quarantine.lock().unwrap();

                            let maybe_id = e.parse::<usize>();
                            let entry = match maybe_id {
                                Ok(id) => quarantine.get_entry_by_id(id),
                                Err(_) => quarantine.get_entry_by_name(&e),
                            };

                            if let Some(entry) = entry {
                                quarantine.remove_entry(entry);
                                let _ = self
                                    .channels
                                    .borrow()
                                    .get(&cmd.id)
                                    .unwrap()
                                    .send(CommandResult::QuarantineAction(true));
                            } else {
                                let _ = self
                                    .channels
                                    .borrow()
                                    .get(&cmd.id)
                                    .unwrap()
                                    .send(CommandResult::QuarantineAction(false));
                            }
                        }
                        None => {
                            let _ = self
                                .channels
                                .borrow()
                                .get(&cmd.id)
                                .unwrap()
                                .send(CommandResult::QuarantineAction(false));
                        }
                    },
                },
                Err(e) => {
                    error!("error receiving command for detector: {}", e);
                }
            }
        }
    }

    pub(crate) fn add_positive_action(&mut self, callback: Box<dyn Fn(&DetectionDetails) + Send>) {
        self.positive_detection_action
            .push(Arc::new(Mutex::new(callback)));
    }

    // This cannot make file accesses otherwise it will block itself
    fn detector_callback(&self, event_meta: &fanotify_event_metadata) -> FanotifyEventResponse {
        if event_meta.pid as u32 == self.daemon_pid {
            // ignore accesses from myself
            debug!("ignoring access from myself");
            return FanotifyEventResponse::Allow;
        }

        let detect_start_ts = Instant::now();
        /// SAFETY: If fanotify does not return a valid filedescriptor, we have bigger
        /// problems than invalid handles in rust
        let mut file = unsafe { File::from_raw_fd(event_meta.fd) };
        let maybe_filename = simbiota_monitor::get_filename_from_fd(event_meta.fd);
        let has_filename = maybe_filename.is_some();
        let filename = maybe_filename.unwrap_or_else(|| "<n/a>".to_string());
        let orig_fname = filename.clone();

        info!("checking file: {}", filename);
        // check cache first
        if has_filename {
            if let Some(result) = self.cache.borrow().get_result_for(&filename, event_meta) {
                let detection_duration = detect_start_ts.elapsed();

                debug!(
                    "scanning took: {:?} (cached)",
                    detection_duration.clone()
                );
                return if result == DetectionResult::NoMatch {
                    info!(
                        "detection negative: {} (cached)",
                        filename
                    );
                    Allow
                } else {
                    error!("detection positive: {} (cached)", filename);
                    self.file_detected_action(filename.clone());
                    Deny
                };
            }
        }
        let mut no_cache = false;
        let mut res = self
            .detector
            .borrow_mut()
            .check_reader(&mut file)
            .unwrap_or_else(|e| {
                warn!("error checking file: {} ({})", filename, e);
                no_cache = true; // skip caching this result
                DetectionResult::NoMatch
            });

        let detection_duration = detect_start_ts.elapsed();
        debug!(
            "scanning took: {:?}",
            detection_duration.clone()
        );

        if !no_cache {
            self.cache
                .borrow_mut()
                .set_result_for(orig_fname.clone(), event_meta, res);
        }

        if res == DetectionResult::Match {
            error!("detection positive: {}", filename);
            self.file_detected_action(orig_fname);
            debug!("detected actions done");
        } else {
            info!("detection negative: {}", filename);
        }

        debug!(
            "blocking took: {:?}",
            detect_start_ts.elapsed()
        );
        if res == DetectionResult::Match {
            Deny
        } else {
            Allow
        }
    }

    fn file_detected_action(&self, filename: String) {
        let actions = self.positive_detection_action.clone();
        let quarantine = self.quarantine.clone();
        thread::spawn(move || {
            let callbacks = actions;
            let detection_details = DetectionDetails {
                path: filename.clone(),
                time: chrono::Utc::now(),
            };

            if let Some(quarantine) = &quarantine {
                error!("moving file to quarantine: {}", filename);
                quarantine.lock().unwrap().add_file(&filename);
            } else {
                info!(
                    "not moving file to quarantine: quarantine disabled"
                );
            }

            for positive_callback in callbacks {
                (positive_callback.lock().unwrap())(&detection_details);
            }
            trace!("finished callbacks");
        });
    }
}

fn is_cache_disabled(config: &DaemonConfig) -> bool {
    let Some(cache_cfg) = &config.cache else {
        return false;
    };
    cache_cfg.disable_cache
}
