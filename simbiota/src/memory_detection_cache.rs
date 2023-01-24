use libc::{c_uint, fanotify_event_metadata, fstat, gid_t, mode_t, off_t, stat, time_t, uid_t};
use log::debug;
use simbiota_clientlib::api::cache::DetectionCache;
use simbiota_clientlib::api::detector::DetectionResult;
use std::collections::HashMap;

struct MemoryCacheEntry {
    pub data: StatBasedCacheData,
    pub result: DetectionResult,
}

#[derive(Eq, PartialEq)]
struct StatBasedCacheData {
    size: off_t,
    uid: uid_t,
    gid: gid_t,
    mtime: time_t,
    ctime: time_t,
    mode: mode_t,
}

impl From<fanotify_event_metadata> for StatBasedCacheData {
    fn from(value: fanotify_event_metadata) -> Self {
        let mut stat = std::mem::MaybeUninit::<stat>::zeroed();
        /// SAFETY: Normal LibC call, stat is a zeroed struct
        let stat_res = unsafe { fstat(value.fd, stat.as_mut_ptr()) };
        if stat_res < 0 {
            panic!("stat failed");
        }
        /// SAFETY: If `fstat` failed, we paniced before, if it has not, we are OK
        let stat = unsafe { stat.assume_init() };
        Self {
            size: stat.st_size,
            uid: stat.st_uid,
            gid: stat.st_gid,
            mtime: stat.st_mtime,
            ctime: stat.st_ctime,
            mode: stat.st_mode,
        }
    }
}

pub struct MemoryDetectionCache {
    cache_map: HashMap<String, MemoryCacheEntry>,
}

impl MemoryDetectionCache {
    pub fn new() -> Self {
        Self {
            cache_map: HashMap::new(),
        }
    }
}

impl DetectionCache<fanotify_event_metadata> for MemoryDetectionCache {
    fn get_result_for(
        &self,
        key: &str,
        event_meta: &fanotify_event_metadata,
    ) -> Option<DetectionResult> {
        let Some(entry) = self.cache_map.get(key) else {
            return None
        };
        let current_data = StatBasedCacheData::from(*event_meta);
        if current_data == entry.data {
            return Some(entry.result);
        }
        None
    }

    fn set_result_for(
        &mut self,
        key: String,
        data: &fanotify_event_metadata,
        result: DetectionResult,
    ) {
        let current_data = StatBasedCacheData::from(*data);
        self.cache_map.insert(
            key,
            MemoryCacheEntry {
                data: current_data,
                result,
            },
        );
        if cfg!(debug_log) {
            let cache_size = self.cache_map.keys().len() * std::mem::size_of::<MemoryCacheEntry>();
            debug!("cache size is {} bytes + keys", cache_size);
        }
    }
}
