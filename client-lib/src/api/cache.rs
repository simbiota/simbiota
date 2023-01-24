use crate::api::detector::DetectionResult;

/// Caching system for detection implementations.
///
/// It can be used for speeding up the blocking operation by
/// only scanning files that were not scanned or were modified.
///
/// The planned implementation uses `fanotify_event_metadata` for `T`.
pub trait DetectionCache<T> {
    fn get_result_for(&self, key: &str, data: &T) -> Option<DetectionResult>;
    fn set_result_for(&mut self, key: String, data: &T, result: DetectionResult);
}

/// Simple cache implementation that does nothing
///
/// Can be used to disable cacheing
pub struct NoopCache;
impl<T> DetectionCache<T> for NoopCache {
    fn get_result_for(&self, _key: &str, _data: &T) -> Option<DetectionResult> {
        None
    }

    fn set_result_for(&mut self, _key: String, _data: &T, _result: DetectionResult) {
        // noop
    }
}
