#[cfg(target_os = "linux")]
pub mod low_level_linux;

use libc::{FAN_ACCESS_PERM, FAN_OPEN_PERM};
#[cfg(target_os = "linux")]
pub use low_level_linux::*;

#[cfg(not(target_os = "linux"))]
pub mod low_level_dummy;
#[cfg(not(target_os = "linux"))]
pub use low_level_dummy::*;

/// Missing from LibC, stolen from Linux source code (v5.15.89)
/// Note: IMO we should try to upstream this to https://github.com/rust-lang/libc
pub const FAN_OPEN_EXEC: u64 = 0x0000_1000;
pub const FAN_OPEN_EXEC_PERM: u64 = 0x0004_0000;
pub const FANOTIFY_PERM_EVENTS: u64 = FAN_OPEN_EXEC_PERM | FAN_ACCESS_PERM | FAN_OPEN_PERM;
