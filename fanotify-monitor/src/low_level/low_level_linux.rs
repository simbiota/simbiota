//! Implement low-level but safe fanotify API using the libc crate
//! See [here](https://man7.org/linux/man-pages/man7/fanotify.7.html) for more info about fanotify

use crate::low_level::FanotifyMarkError::{
    AllocationError, InvalidValue, NotADirectory, NotExists, OperationNotSupported, OutOfMarks,
    UnknownError, UnsupportedFilesystem,
};
use crate::low_level::FANOTIFY_PERM_EVENTS;
use crate::FanotifyEventResponse;
use crate::FanotifyEventResponse::Allow;
use crossbeam_channel::{Receiver, Sender};
use libc::{
    c_uint, fanotify_event_metadata, fanotify_init, fanotify_mark, fanotify_response, perror, poll,
    pollfd, read, ssize_t, write, AT_FDCWD, EINVAL, EMFILE, ENODEV, ENOENT, ENOMEM, ENOSPC, ENOSYS,
    ENOTDIR, EOPNOTSUPP, EPERM, EXDEV, FAN_ALLOW, FAN_CLASS_CONTENT, FAN_CLASS_NOTIF,
    FAN_CLASS_PRE_CONTENT, FAN_DENY, POLLIN,
};
use log::warn;
use std::ffi::{c_void, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;

#[warn(non_camel_case_types)]
/// Used for fanotify_init to indicate which content setting the caller want to use for
/// the events
pub enum FANClass {
    /// Access content before it is final, mainly for storage managers
    ClassPreContent,
    /// Access content after it is final, used for malware detection
    ClassContent,
    /// No content can be read
    ClassNotif,
}

impl FANClass {
    pub fn as_libc(&self) -> u32 {
        match self {
            FANClass::ClassContent => FAN_CLASS_CONTENT,
            FANClass::ClassPreContent => FAN_CLASS_PRE_CONTENT,
            FANClass::ClassNotif => FAN_CLASS_NOTIF,
        }
    }
}

/// Possible errors for [`monitor_init()`].
///
/// These errors can occur when the fanotify_init is called.
#[derive(Debug)]
pub enum FanotifyInitError {
    /// EINVAL: Invalid class or event_flag.
    InvalidArguments,
    /// EMFILE: Number of fanotify groups(128) reached or per-process fd limit reached.
    LimitReached,
    /// ENOMEM: Group allocation failed.
    AllocationError,
    /// ENOSYS: The kernel does not implement fanotify_init. Recompile the kernel with CONFIG_FANOTIFY.
    NotImplemented,
    /// EPERM: The process does not have CAP_SYS_ADMIN.
    InsufficientPermission,
    /// Unknown error: The errno is provided.
    UnknownError { errno: i32 },
}

#[derive(Debug)]
pub enum FanotifyMarkError {
    InvalidFileDescriptor,
    InvalidValue,
    UnsupportedFilesystem,
    NotExists,
    AllocationError,
    OutOfMarks,
    NotImplemented,
    NotADirectory,
    OperationNotSupported,
    EXDEV,
    UnknownError { errno: i32 },
}

#[repr(transparent)]
pub struct FanotifyDescriptor {
    pub(crate) fd: i32,
}

/// Initializes a fanotify group with the provided parameters
///
/// Returns a valid file descriptor on success, or an error type if the initialization failed.
///
/// Safety: This methods is safe, does nothing related to memory access. Use O_CLOEXEC in
/// multithreaded environment!
pub fn monitor_init(
    class: FANClass,
    flags: u32,
    event_flags: u32,
) -> Result<FanotifyDescriptor, FanotifyInitError> {
    // SAFETY: LibC call
    let init_result = unsafe { fanotify_init(class.as_libc() | flags, event_flags) };

    if init_result != -1 {
        Ok(FanotifyDescriptor { fd: init_result })
    } else {
        let error_code = std::io::Error::last_os_error().raw_os_error().unwrap();
        Err(match error_code {
            EINVAL => FanotifyInitError::InvalidArguments,
            EMFILE => FanotifyInitError::LimitReached,
            ENOMEM => FanotifyInitError::AllocationError,
            ENOSYS => FanotifyInitError::NotImplemented,
            EPERM => FanotifyInitError::InsufficientPermission,
            number => FanotifyInitError::UnknownError { errno: number },
        })
    }
}

/// `fanotify_mark`s a path
pub fn monitor_mark(
    fanotify_fd: &FanotifyDescriptor,
    flags: u64,
    mask: u64,
    dirfd: i32,
    path: &Path,
) -> Result<(), FanotifyMarkError> {
    let flag_val = flags;
    let dirfd = if dirfd == 0 { AT_FDCWD } else { dirfd };
    let path_string = CString::new(path.as_os_str().as_bytes()).expect("cannot be used as path");
    let retval = unsafe {
        fanotify_mark(
            fanotify_fd.fd,
            flag_val as c_uint,
            mask,
            dirfd,
            path_string.as_ptr(),
        )
    };
    if retval == 0 {
        return Ok(());
    }

    let errno = std::io::Error::last_os_error().raw_os_error().unwrap();
    Err(match errno {
        libc::EBADF => FanotifyMarkError::InvalidFileDescriptor,
        EINVAL => InvalidValue,
        ENODEV => UnsupportedFilesystem,
        ENOENT => NotExists,
        ENOMEM => AllocationError,
        ENOSPC => OutOfMarks,
        ENOSYS => OperationNotSupported,
        ENOTDIR => NotADirectory,
        EOPNOTSUPP => OperationNotSupported,
        EXDEV => FanotifyMarkError::EXDEV,
        v => UnknownError { errno: v },
    })
}

/// Closes the fanotify descriptor
pub fn monitor_close(fanotify_fd: FanotifyDescriptor) -> Result<(), i32> {
    let result = unsafe { libc::close(fanotify_fd.fd) };
    if result == -1 {
        return Err(std::io::Error::last_os_error().raw_os_error().unwrap());
    }
    Ok(())
}

const MSG_BUFFER_SIZE: usize = 1024;

struct FanotifyEventIterator<'a> {
    read_len: ssize_t,
    data_buffer: &'a [u8],
    start_ptr: *const fanotify_event_metadata,
}

impl<'a> Iterator for FanotifyEventIterator<'a> {
    type Item = &'a fanotify_event_metadata;

    // #define FAN_EVENT_OK(meta, len)	((long)(len) >= (long)FAN_EVENT_METADATA_LEN && \
    // 				(long)(meta)->event_len >= (long)FAN_EVENT_METADATA_LEN && \
    // 				(long)(meta)->event_len <= (long)(len))

    // #define FAN_EVENT_NEXT(meta, len) ((len) -= (meta)->event_len, \
    //              (struct fanotify_event_metadata*)(((char *)(meta)) + \
    //              (meta)->event_len))

    /// Safety: based on the official macros for processing fanotify events.
    /// However, as it is simple pointer math and raw arrays, it is inherently unsafe
    /// and works on a best effort basis.
    fn next(&mut self) -> Option<Self::Item> {
        if self.read_len >= std::mem::size_of::<fanotify_event_metadata>() as isize {
            if self.start_ptr.is_null() {
                self.start_ptr =
                    self.data_buffer.as_ptr() as *const _ as *const fanotify_event_metadata;
            }
            if unsafe {
                (*self.start_ptr).event_len >= std::mem::size_of::<fanotify_event_metadata>() as u32
                    && (*self.start_ptr).event_len <= self.read_len as u32
            } {
                let current_item = unsafe { &*self.start_ptr };

                // FAN_EVENT_NEXT
                self.read_len -= current_item.event_len as isize;
                self.start_ptr = unsafe { self.start_ptr.add(current_item.event_len as usize) };
                return Some(current_item);
            }
        }
        None
    }
}

impl FanotifyEventResponse {
    pub fn as_libc(&self) -> u32 {
        match self {
            FanotifyEventResponse::Allow => FAN_ALLOW,
            FanotifyEventResponse::Deny => FAN_DENY,
        }
    }
}

pub type MonitorResponseCallback =
    Arc<dyn Fn(&fanotify_event_metadata) -> FanotifyEventResponse + Send + Sync>;
pub type MonitorEventCallback = Arc<dyn Fn(&fanotify_event_metadata) + Send + Sync>;

struct MonitorResponder {
    receiver: Receiver<MonitorEvent>,
    fd: i32,
    response_callback: MonitorResponseCallback,
    event_callback: MonitorEventCallback,
    write_lock: Arc<Mutex<()>>,
}

enum MonitorEvent {
    PermEvent(fanotify_event_metadata),
    NormalEvent(fanotify_event_metadata),
}

impl MonitorResponder {
    pub fn new(
        fd: i32,
        response_callback: MonitorResponseCallback,
        event_callback: MonitorEventCallback,
        write_lock: Arc<Mutex<()>>,
    ) -> (Self, Sender<MonitorEvent>) {
        let (sender, receiver) = crossbeam_channel::unbounded();
        (
            Self {
                receiver,
                fd,
                event_callback,
                response_callback,
                write_lock,
            },
            sender,
        )
    }

    pub fn start(&self) -> ! {
        loop {
            let data = self.receiver.recv();
            match data {
                Ok(event) => match event {
                    MonitorEvent::PermEvent(meta) => {
                        let result = self.response_callback.as_ref()(&meta);
                        let resp = fanotify_response {
                            fd: meta.fd,
                            response: result.as_libc(),
                        };

                        let _lock = self.write_lock.lock().unwrap();
                        let write_res = unsafe {
                            write(
                                self.fd,
                                (&resp) as *const _ as *const c_void,
                                std::mem::size_of::<fanotify_response>(),
                            )
                        };
                        if write_res < 0 {
                            panic!("response write failed");
                        }
                    }
                    MonitorEvent::NormalEvent(meta) => {
                        self.event_callback.as_ref()(&meta);
                    }
                },
                Err(e) => panic!("monitor responder died: {}", e),
            }
        }
    }
}

/// Start listening to the fanotify monitor using the `poll` function
///
/// Safety: This has to be tested extensively as the current implementation _can_ be overwhelmed. The
/// memory safety is depending on the kernel's ability to process a huge number of PERM events
pub fn monitor_listen(
    fanotify_fd: &FanotifyDescriptor,
    response_callback: MonitorResponseCallback,
    event_callback: MonitorEventCallback,
) -> ! {
    let poll_array = [pollfd {
        fd: fanotify_fd.fd,
        events: POLLIN,
        revents: 0,
    }; 1];

    let mut msg_buffer: [u8; MSG_BUFFER_SIZE] = [0; MSG_BUFFER_SIZE];
    let mypid = unsafe { libc::getpid() };
    let write_lock = Arc::new(Mutex::new(()));
    let (processor, sender) = MonitorResponder::new(
        fanotify_fd.fd,
        response_callback,
        event_callback,
        write_lock.clone(),
    );

    thread::Builder::new()
        .name("MonitorResponder".to_string())
        .spawn(move || {
            processor.start();
        })
        .unwrap();

    loop {
        unsafe {
            if poll(poll_array.as_ptr() as *mut pollfd, 1, -1) < 0 {
                let error = CString::new("poll()").unwrap();
                perror(error.as_ptr());
            }
        }

        unsafe {
            if poll_array[0].revents & POLLIN > 0 {
                let read_len = read(
                    poll_array[0].fd,
                    msg_buffer.as_mut_ptr() as *mut c_void,
                    MSG_BUFFER_SIZE,
                );
                if read_len > 0 {
                    let event_iterator = FanotifyEventIterator {
                        read_len,
                        data_buffer: &msg_buffer,
                        start_ptr: std::ptr::null(),
                    };
                    for event_meta in event_iterator {
                        if event_meta.mask & FANOTIFY_PERM_EVENTS > 0 {
                            let pid = event_meta.pid;
                            // Always allow events from this process
                            if pid == mypid {
                                let _lock = write_lock.lock().unwrap();
                                let resp = fanotify_response {
                                    fd: event_meta.fd,
                                    response: Allow.as_libc(),
                                };

                                let write_res = write(
                                    fanotify_fd.fd,
                                    (&resp) as *const _ as *const c_void,
                                    std::mem::size_of::<fanotify_response>(),
                                );
                                if write_res < 0 {
                                    let error = CString::new("write").unwrap();
                                    perror(error.as_ptr());
                                    panic!("response write failed");
                                }
                            } else {
                                let event_meta = *event_meta;
                                sender.send(MonitorEvent::PermEvent(event_meta)).unwrap();
                            }
                        } else {
                            let event_meta = *event_meta;
                            sender.send(MonitorEvent::NormalEvent(event_meta)).unwrap();
                        }
                    }
                }
            }
        }
    }
}
