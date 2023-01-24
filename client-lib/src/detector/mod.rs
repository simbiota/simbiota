use crate::api::detector::Detector;
use crate::system_database::SystemDatabase;
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub mod tlsh_detector;

pub trait DetectorProvider {
    fn get_detector(
        &self,
        configuration: &HashMap<String, Box<dyn Any>>,
        database: Arc<Mutex<SystemDatabase>>,
    ) -> Box<dyn Detector>;
}
