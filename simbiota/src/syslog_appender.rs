use crate::syslog_appender;
use crate::syslog_appender::SyslogFormat::{Format3164, Format5424};
use log::{Level, Log, Record};
use log4rs::append::Append;
use log4rs::config::Appender;
use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::io::Write;
use std::sync::Mutex;
use syslog::{Facility, Formatter3164, Formatter5424, Logger, LoggerBackend};

#[derive(Debug)]
pub enum SyslogFormat {
    Format3164,
    Format5424,
}

trait Syslog: Send + Sync + Debug {
    fn log(&mut self, record: &Record) -> anyhow::Result<()>;
    fn flush(&mut self);
}

struct Syslog3164 {
    log: Logger<LoggerBackend, Formatter3164>,
}

impl Syslog3164 {
    pub fn new(facility: Facility) -> Self {
        let formatter = Formatter3164 {
            facility,
            hostname: None,
            process: "simbiota".to_string(),
            pid: std::process::id(),
        };
        Self {
            log: syslog::unix(formatter).unwrap(),
        }
    }
}
impl Debug for Syslog3164 {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Syslog3164")
    }
}

macro_rules! logme {
    ($slf:expr, $fun:tt, $record:expr) => {
        $slf.log.$fun(format!("{}", $record.args())).unwrap();
    };
}
impl Syslog for Syslog3164 {
    fn log(&mut self, record: &Record) -> anyhow::Result<()> {
        match record.level() {
            Level::Error => {
                logme!(self, err, record);
                Ok(())
            }
            Level::Warn => {
                logme!(self, err, record);
                Ok(())
            }
            Level::Info => {
                logme!(self, info, record);
                Ok(())
            }
            Level::Debug => {
                logme!(self, debug, record);
                Ok(())
            }
            Level::Trace => {
                logme!(self, notice, record);
                Ok(())
            }
        }
    }

    fn flush(&mut self) {
        self.log.backend.flush();
    }
}

#[derive(Debug)]
pub struct SyslogAppender {
    logger: Mutex<Box<dyn Syslog>>,
}

impl SyslogAppender {
    pub fn new(facility: Facility, format: SyslogFormat) -> Self {
        let logger: Box<dyn Syslog> = match format {
            syslog_appender::SyslogFormat::Format3164 => Box::new(Syslog3164::new(facility)),
            //syslog_appender::SyslogFormat::Format5424 => Box::new(Syslog5424::new(facility)),
            syslog_appender::SyslogFormat::Format5424 => {
                panic!("currently not supported syslog format")
            }
        };

        Self {
            logger: Mutex::from(logger),
        }
    }
}

impl Append for SyslogAppender {
    fn append(&self, record: &Record) -> anyhow::Result<()> {
        self.logger.lock().unwrap().log(record)
    }

    fn flush(&self) {
        self.logger.lock().unwrap().flush();
    }
}
