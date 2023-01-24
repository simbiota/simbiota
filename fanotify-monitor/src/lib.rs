pub use libc::c_int;
use std::fs::read_link;
use std::path::Path;

mod low_level;
pub mod monitor;

pub enum FanotifyEventResponse {
    Allow,
    Deny,
}

pub fn get_filename_from_fd(fd: c_int) -> Option<String> {
    let path_str = format!("/proc/self/fd/{}", fd);
    let path = Path::new(path_str.as_str());

    if path.exists() {
        let Ok(target) = read_link(path) else  { return None };
        return Some(target.display().to_string());
    }

    None
}
