use log::{LevelFilter, Log, Metadata, Record};
use std::cell::RefCell;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::sync::Mutex;

/// Wrapper for runtime changeable logger implementations.
///
/// SAFETY: The instance of [`SimbiotaLoggerHolder`] containing the currently used logger
/// **MUST BE** kept in memory for the entirety of logging usage. If the instance dropped,
/// the behaviour of _all_ logger calls are undefined.
///
/// # Intended usage
/// The instance is intentionally leaked, it lives until the end of the program. This way, it is not
/// possible to run into UB with logging calls.
/// ```
/// let mut logger_holder = Box::leak(Box::new(SimbiotaLoggerHolder::new())); // logger_holder is &'static SimbiotaLoggerHolder
/// ```
pub(crate) struct SimbiotaLoggerHolder {
    inner: Mutex<Option<Box<SimbiotaLogger>>>,
}

impl SimbiotaLoggerHolder {
    pub fn set_logger(&self, logger: Box<dyn Log>) {
        if self.inner.lock().unwrap().is_none() {
            let mut inner = self.inner.lock().unwrap();
            *inner = Some(Box::new(SimbiotaLogger::new()));
        }
        let lock = self.inner.lock().unwrap();
        let inner = lock.as_ref().unwrap();
        let was_empty = inner.current_logger.lock().unwrap().is_none();
        let mut cl = inner.current_logger.lock().unwrap();
        *cl = Some(logger);

        if was_empty {
            let box_ref = lock.as_ref();
            let boxed = box_ref.unwrap();
            log::set_max_level(LevelFilter::Trace);
            /// SAFETY: Just an ugly hack to get the same instance. Safe as long as the current instance
            /// is in memory
            unsafe {
                // This is ugly AF
                let logger: *const SimbiotaLogger = boxed.as_ref();
                let static_living: &'static SimbiotaLogger = &*logger;
                log::set_logger_racy(static_living).unwrap();
            }
        }
    }

    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }
}

struct SimbiotaLogger {
    current_logger: Mutex<Option<Box<dyn Log>>>,
}

impl SimbiotaLogger {
    pub const fn new() -> Self {
        Self {
            current_logger: Mutex::new(None),
        }
    }

    pub fn set_logger(&self, logger: Box<dyn Log>) {
        let was_empty = self.current_logger.lock().unwrap().is_none();
        let mut cl = self.current_logger.lock().unwrap();
        *cl = Some(logger);
        if was_empty {}
    }
}

impl log::Log for SimbiotaLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.current_logger
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .enabled(metadata)
    }

    fn log(&self, record: &Record) {
        self.current_logger
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .log(record);
    }

    fn flush(&self) {
        self.current_logger
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .flush();
    }
}
