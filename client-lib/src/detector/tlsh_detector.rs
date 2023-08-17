use log::{debug, warn};

use crate::api::detector::Detector;
use crate::api::hash::{
    AbstractHashBasedDetector, ComparableHash, CompareAgainstAllDetector, HashAlg, HashDatabase,
};
use crate::detector::DetectorProvider;
use crate::system_database::{SystemDatabase, SystemDatabaseObject};
use simbiota_database::formats::colored_tlsh::ColoredTLSHObject;
use simbiota_database::formats::colored_tlsh_with_distance::ColoredTLSHWithDistanceObject;
use simbiota_database::ObjectImpl;
use simbiota_tlsh::{TLSHBuilder, TLSH};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct ComparableTLSHHash {
    pub(crate) inner: TLSH,
    detection_distance: u8,
}

impl ComparableTLSHHash {
    pub fn detection_distance(&self) -> u8 {
        if self.detection_distance == 0 {
            panic!("detection distance not set");
        }
        self.detection_distance
    }
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
        Some(ComparableTLSHHash {
            inner: raw_hash,
            detection_distance: 0,
        })
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
        let mut system_database = system_database.lock().unwrap();
        let comparator = if let Some(object) =
            system_database.get_object::<ColoredTLSHWithDistanceObject>(0x0003)
        {
            let database = DistancedTLSHDatabase::new(object);
            CompareAgainstAllDetector::new(
                Box::new(database),
                Box::new(move |hash, stored_hash| {
                    let diff = stored_hash.diff(hash);
                    if diff < stored_hash.detection_distance() as i32 {
                        debug!(
                            "TLSH below threshold: {} < {}",
                            diff,
                            stored_hash.detection_distance()
                        );
                        return true;
                    }
                    false
                }),
            )
        } else if let Some(legacy_object) = system_database.get_object::<ColoredTLSHObject>(0x0002)
        {
            warn!("using legacy database format, please update the database");
            let database = LegacyTLSHDatabase::new(legacy_object);
            let threshold = if let Some(threshold) = configuration.get("threshold") {
                let Some(threshold) = threshold.downcast_ref::<i64>() else {
                        panic!("invalid threshold config")
                    };
                *threshold as i32
            } else {
                40
            };
            CompareAgainstAllDetector::new(
                Box::new(database),
                Box::new(move |hash, stored_hash| {
                    let diff = stored_hash.diff(hash);
                    if diff < threshold {
                        debug!("TLSH below threshold: {} < {threshold}", diff);
                        return true;
                    }
                    false
                }),
            )
        } else {
            panic!(
                "no usable object found in database. Please update the database to a later version"
            )
        };
        let detector: AbstractHashBasedDetector<TLSHHashAlg, ComparableTLSHHash> =
            AbstractHashBasedDetector::new(Box::from(comparator));

        Box::new(detector)
    }
}

pub(crate) struct DistancedTLSHDatabase {
    sdo: Arc<SystemDatabaseObject>,
    hashes: HashMap<u8, Vec<ComparableTLSHHash>>,
}

impl HashDatabase<ComparableTLSHHash> for DistancedTLSHDatabase {
    fn get_hashes(&mut self) -> &[ComparableTLSHHash] {
        if self.sdo.has_changed() {
            self.reload();
        }
        self.hashes[&0u8].as_slice()
    }
}

impl DistancedTLSHDatabase {
    pub fn reload(&mut self) {
        debug!("Reloading TLSH store");
        self.hashes.clear();

        let object = self.sdo.object().lock().unwrap().clone();
        let tlsh_obj =
            ColoredTLSHWithDistanceObject::from_object(object).expect("invalid database object");

        for hash in tlsh_obj.get_entries() {
            let tlsh_hash = TLSH::from_raw(&hash.tlsh_bytes);
            self.hashes
                .entry(tlsh_hash.color)
                .or_insert_with(Default::default);
            let colored_hashes = self.hashes.get_mut(&tlsh_hash.color).unwrap();
            colored_hashes.push(ComparableTLSHHash {
                inner: tlsh_hash,
                detection_distance: hash.distance,
            });
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

pub(crate) struct LegacyTLSHDatabase {
    sdo: Arc<SystemDatabaseObject>,
    hashes: HashMap<u8, Vec<ComparableTLSHHash>>,
}

impl HashDatabase<ComparableTLSHHash> for LegacyTLSHDatabase {
    fn get_hashes(&mut self) -> &[ComparableTLSHHash] {
        if self.sdo.has_changed() {
            self.reload();
        }
        self.hashes[&0u8].as_slice()
    }
}

impl LegacyTLSHDatabase {
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
            colored_hashes.push(ComparableTLSHHash {
                inner: tlsh_hash,
                detection_distance: 0,
            });
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
