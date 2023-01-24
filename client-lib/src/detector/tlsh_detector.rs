use log::debug;

use crate::api::detector::Detector;
use crate::api::hash::{
    AbstractHashBasedDetector, ComparableHash, CompareAgainstAllDetector, HashAlg, HashDatabase,
};
use crate::detector::DetectorProvider;
use crate::system_database::{SystemDatabase, SystemDatabaseObject};
use simbiota_database::formats::colored_tlsh::ColoredTLSHObject;
use simbiota_database::ObjectImpl;
use simbiota_tlsh::{TLSHBuilder, TLSH};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct ComparableTLSHHash {
    pub(crate) inner: TLSH,
}

pub struct TLSHHashAlg {
    builder: TLSHBuilder,
}

impl HashAlg<ComparableTLSHHash> for TLSHHashAlg {
    fn new() -> Self {
        Self {
            builder: TLSHBuilder::default(),
        }
    }

    fn update(&mut self, data_buffer: &[u8]) {
        self.builder.update(data_buffer)
    }

    fn finalize(&mut self) {
        self.builder.finalize()
    }

    fn get_hash(&self) -> Option<ComparableTLSHHash> {
        let Ok(raw_hash) = self.builder.get_hashes()[0] else {
            return None;
        };
        debug!("TLSH hash: {}", raw_hash.to_digest());
        Some(ComparableTLSHHash { inner: raw_hash })
    }
}

impl ComparableHash for ComparableTLSHHash {
    type ResultType = i32;

    fn diff(&self, other: &Self) -> Self::ResultType {
        TLSH::diff(&self.inner, &other.inner)
    }

    fn get_digest(&self) -> Box<[u8]> {
        Box::from(self.inner.to_raw())
    }

    fn get_digest_hex(&self) -> String {
        self.inner.to_digest()
    }

    fn color(&self) -> u8 {
        self.inner.color
    }
}

pub struct SimpleTLSHDetectorProvider;
impl SimpleTLSHDetectorProvider {
    pub fn new() -> Self {
        Self
    }
}
impl DetectorProvider for SimpleTLSHDetectorProvider {
    fn get_detector(
        &self,
        configuration: &HashMap<String, Box<dyn Any>>,
        system_database: Arc<Mutex<SystemDatabase>>,
    ) -> Box<dyn Detector> {
        let threshold = if let Some(threshold) = configuration.get("threshold") {
            let Some(threshold) = threshold.downcast_ref::<i64>() else {
                panic!("invalid threshold config")
            };
            *threshold as i32
        } else {
            40
        };

        let mut system_database = system_database.lock().unwrap();
        let object = system_database
            .get_object::<ColoredTLSHObject>(0x0002)
            .expect("cannot construct TLSH detector from empty database");
        let database = SimpleTLSHDatabase::new(object);
        let comparator = CompareAgainstAllDetector::new(
            Box::new(database),
            Box::new(move |diff| {
                let result = diff < threshold;
                if result {
                    debug!("TLSH below threshold: {} < {}", diff, threshold);
                }
                result
            }),
        );
        let detector: AbstractHashBasedDetector<TLSHHashAlg, ComparableTLSHHash> =
            AbstractHashBasedDetector::new(Box::from(comparator));

        Box::new(detector)
    }
}

pub(crate) struct SimpleTLSHDatabase {
    sdo: Arc<SystemDatabaseObject>,
    hashes: HashMap<u8, Vec<ComparableTLSHHash>>,
}

impl HashDatabase<ComparableTLSHHash> for SimpleTLSHDatabase {
    fn get_hashes(&mut self) -> &[ComparableTLSHHash] {
        if self.sdo.has_changed() {
            self.reload();
        }
        self.hashes[&0u8].as_slice()
    }
}

impl SimpleTLSHDatabase {
    pub fn reload(&mut self) {
        debug!("Reloading TLSH store");
        self.hashes.clear();

        let object = self.sdo.object().lock().unwrap().clone();
        let tlsh_obj = ColoredTLSHObject::from_object(object).expect("invalid database object");

        for hash in tlsh_obj.get_entries() {
            let tlsh_hash = TLSH::from_raw(&hash.tlsh_bytes);
            self.hashes
                .entry(tlsh_hash.color)
                .or_insert_with(Default::default);
            let colored_hashes = self.hashes.get_mut(&tlsh_hash.color).unwrap();
            colored_hashes.push(ComparableTLSHHash { inner: tlsh_hash });
        }
        debug!("{} hashes in database", self.hashes[&0].len());
    }

    pub fn new(sdo: Arc<SystemDatabaseObject>) -> Self {
        let mut db = Self {
            sdo,
            hashes: HashMap::new(),
        };
        db.reload();
        db
    }
}
