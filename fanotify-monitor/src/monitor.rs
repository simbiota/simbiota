use crate::low_level::{
    monitor_close, monitor_init, monitor_listen, monitor_mark, FanotifyDescriptor, FAN_OPEN_EXEC,
    FAN_OPEN_EXEC_PERM,
};

pub use crate::low_level::{FanotifyInitError, FanotifyMarkError};
use crate::FanotifyEventResponse;
use libc::{
    c_int, c_uint, AT_FDCWD, FAN_ACCESS, FAN_ACCESS_PERM, FAN_CLOEXEC, FAN_CLOSE_NOWRITE,
    FAN_CLOSE_WRITE, FAN_EVENT_ON_CHILD, FAN_MARK_ADD, FAN_MARK_DONT_FOLLOW, FAN_MARK_FILESYSTEM,
    FAN_MARK_FLUSH, FAN_MARK_IGNORED_MASK, FAN_MARK_IGNORED_SURV_MODIFY, FAN_MARK_MOUNT,
    FAN_MARK_ONLYDIR, FAN_MARK_REMOVE, FAN_MODIFY, FAN_NONBLOCK, FAN_ONDIR, FAN_OPEN,
    FAN_OPEN_PERM, FAN_UNLIMITED_MARKS, FAN_UNLIMITED_QUEUE, O_APPEND, O_CLOEXEC, O_DSYNC,
    O_LARGEFILE, O_NOATIME, O_NONBLOCK, O_RDONLY, O_RDWR, O_SYNC, O_WRONLY,
};

pub use libc::fanotify_event_metadata;

use bitflags::bitflags;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use crate::low_level::FANClass;

pub struct FilesystemMonitor {
    fanotify_fd: FanotifyDescriptor,

    paths_to_add: Vec<(MarkFlags, EventMask, PathBuf)>,
}

impl Drop for FilesystemMonitor {
    fn drop(&mut self) {
        monitor_close(FanotifyDescriptor {
            fd: self.fanotify_fd.fd,
        })
        .expect("FilesystemMonitor failed to close the file descriptor");
    }
}

impl FilesystemMonitor {
    pub fn new(
        class: FANClass,
        monitor_flags: MonitorFlags,
        event_flags: EventFlags,
    ) -> Result<Self, FanotifyInitError> {
        let monitor_fd = monitor_init(class, monitor_flags.bits, event_flags.bits as u32)?;

        Ok(Self {
            fanotify_fd: monitor_fd,
            paths_to_add: Vec::new(),
        })
    }

    pub fn add_path(&mut self, path: &Path, flags: MarkFlags, mask: EventMask) {
        let flags_val = flags | MarkFlags::ADD;
        self.paths_to_add.push((flags_val, mask, path.to_owned()));
    }

    pub fn remove_path(
        &self,
        path: &Path,
        flags: MarkFlags,
        mask: EventMask,
    ) -> Result<(), FanotifyMarkError> {
        let flags_val = flags | MarkFlags::REMOVE;
        self.mark(flags_val, mask, path)
    }

    pub fn flush_path(
        &self,
        path: &Path,
        flags: MarkFlags,
        mask: EventMask,
    ) -> Result<(), FanotifyMarkError> {
        let flags_val = flags | MarkFlags::FLUSH;
        self.mark(flags_val, mask, path)
    }

    fn mark(
        &self,
        flags: MarkFlags,
        mask: EventMask,
        path: &Path,
    ) -> Result<(), FanotifyMarkError> {
        monitor_mark(
            &self.fanotify_fd,
            flags.bits as u64,
            mask.bits,
            AT_FDCWD,
            path,
        )?;
        Ok(())
    }

    pub fn start(
        &self,
        event_callback: Arc<dyn Fn(&fanotify_event_metadata) + Send + Sync>,
        response_callback: Arc<
            dyn Fn(&fanotify_event_metadata) -> FanotifyEventResponse + Send + Sync,
        >,
    ) -> ! {
        for (flags, event, path) in &self.paths_to_add {
            self.mark(*flags, *event, path).unwrap_or_else(|e| {
                if let FanotifyMarkError::InvalidValue = e {
                    if event.contains(EventMask::ACCESS_PERM) | event.contains(EventMask::OPEN_PERM)
                        || event.contains(EventMask::OPEN_EXEC_PERM)
                    {
                        panic!(
                            "failed to mark {}: {e:?}\n\nPlease make sure that CONFIG_FANOTIFY_ACCESS_PERMISSIONS kernel config option is enabled. See docs for more instructions.\n",
                            path.display()
                        )
                    }
                    panic!("failed to mark {}: {e:?}", path.display())
                }
            });
        }
        monitor_listen(&self.fanotify_fd, response_callback, event_callback)
    }
}

bitflags! {
    pub struct EventMask: u64 {
        const ACCESS = FAN_ACCESS;
        const MODIFY = FAN_MODIFY;
        const CLOSE_WRITE = FAN_CLOSE_WRITE;
        const CLOSE_NOWRITE = FAN_CLOSE_NOWRITE;
        const OPEN = FAN_OPEN;
        const OPEN_EXEC = FAN_OPEN_EXEC;
        const OPEN_PERM = FAN_OPEN_PERM;
        const OPEN_EXEC_PERM = FAN_OPEN_EXEC_PERM;
        const ACCESS_PERM = FAN_ACCESS_PERM;
        const ONDIR = FAN_ONDIR;
        const EVENT_ON_CHILD = FAN_EVENT_ON_CHILD;
    }
}

impl EventMask {
    pub fn parse(flags: Vec<&str>) -> Result<Self, String> {
        let mut value = Self::empty();
        for flag in flags {
            match flag.to_uppercase().as_str() {
                "ACCESS" => value.insert(Self::ACCESS),
                "MODIFY" => value.insert(Self::MODIFY),
                "CLOSE_WRITE" => value.insert(Self::CLOSE_WRITE),
                "CLOSE_NOWRITE" => value.insert(Self::CLOSE_NOWRITE),
                "OPEN" => value.insert(Self::OPEN),
                "OPEN_EXEC" => value.insert(Self::OPEN_EXEC),
                "OPEN_PERM" => value.insert(Self::OPEN_PERM),
                "OPEN_EXEC_PERM" => value.insert(Self::OPEN_EXEC_PERM),
                "ACCESS_PERM" => value.insert(Self::ACCESS_PERM),

                _ => return Err(format!("invalid mask: {}", flag)),
            }
        }
        Ok(value)
    }
}

bitflags! {
    pub struct MarkFlags: c_uint {
        const ADD = FAN_MARK_ADD;
        const REMOVE = FAN_MARK_REMOVE;
        const FLUSH = FAN_MARK_FLUSH;
        const DONT_FOLLOW = FAN_MARK_DONT_FOLLOW;
        const ONLY_DIR = FAN_MARK_ONLYDIR;
        const MOUNT = FAN_MARK_MOUNT;
        const FILESYSTEM = FAN_MARK_FILESYSTEM;
        const IGNORED_MASK = FAN_MARK_IGNORED_MASK;
        const IGNORED_SURV_MODIFY = FAN_MARK_IGNORED_SURV_MODIFY;
    }
}

bitflags! {
    pub struct MonitorFlags: c_uint {
        const CLOEXEC = FAN_CLOEXEC;
        const NONBLOCK = FAN_NONBLOCK;
        const UNLIMITED_QUEUE = FAN_UNLIMITED_QUEUE;
        const UNLIMITED_MARKS = FAN_UNLIMITED_MARKS;
    }
}

impl MonitorFlags {
    pub fn parse(flags: Vec<&str>) -> Result<Self, String> {
        let mut value = Self::empty();
        for flag in flags {
            match flag.to_uppercase().as_str() {
                "CLOEXEC" => value.insert(Self::CLOEXEC),
                "NONBLOCK" => value.insert(Self::NONBLOCK),
                "UNLIMITED_QUEUE" => value.insert(Self::UNLIMITED_QUEUE),
                "UNLIMITED_MARKS" => value.insert(Self::UNLIMITED_MARKS),
                _ => return Err(format!("invalid monitor flags: {}", flag)),
            }
        }
        Ok(value)
    }
}

bitflags! {
    pub struct EventFlags : c_int {
        const READONLY = O_RDONLY;
        const WRITEONLY = O_WRONLY;
        const READWRITE = O_RDWR;
        const LARGEFILE = O_LARGEFILE;
        const CLOEXEC = O_CLOEXEC;
        const APPEND = O_APPEND;
        const DSYNC = O_DSYNC;
        const NOATIME = O_NOATIME;
        const NONBLOCK = O_NONBLOCK;
        const SYNC = O_SYNC;
    }
}
