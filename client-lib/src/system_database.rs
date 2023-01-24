use simbiota_database::{Database, LazyLoadedDatabase, Object, ObjectImpl};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::client_config::ClientConfig;
use crate::system_database::DatabaseHolder::{LowMemory, LowMemoryUpdate, Normal};
use log::debug;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

enum DatabaseHolder {
    LowMemory(LazyLoadedDatabase),
    Normal(Database),
    LowMemoryUpdate,
}

pub struct SystemDatabase {
    holder: DatabaseHolder,
    database_path: PathBuf,
    sdos: HashMap<u64, Arc<SystemDatabaseObject>>,
}

impl SystemDatabase {
    pub fn load(config: &ClientConfig) -> Self {
        debug!(
            "loading database from: {}",
            config.database.database_path.display()
        );
        let dbpath = &config.database.database_path;
        if !dbpath.exists() {
            panic!("missing database file");
        }

        let holder = if config.database.low_memory {
            let database = LazyLoadedDatabase::new(dbpath).expect("failed to load database");
            LowMemory(database)
        } else {
            let database_raw = std::fs::read(dbpath).expect("failed to read database file");
            let database =
                Database::from_bytes(database_raw.as_slice()).expect("failed to load database");
            Normal(database)
        };

        Self {
            holder,
            database_path: dbpath.clone(),
            sdos: HashMap::new(),
        }
    }

    pub fn pre_update(&mut self) {
        if let LowMemory(_) = &self.holder {
            let holder = std::mem::replace(&mut self.holder, LowMemoryUpdate);
            let LowMemory(db) = holder else {
                panic!("impossible state");
            };
            db.close();
        }
    }

    pub fn mark_update(&mut self) {
        if let LowMemoryUpdate = &self.holder {
            let database =
                LazyLoadedDatabase::new(&self.database_path).expect("failed to load database");
            self.holder = LowMemory(database)
        } else if let Normal(_) = &self.holder {
            let database_raw =
                std::fs::read(&self.database_path).expect("failed to read database file");
            let database =
                Database::from_bytes(database_raw.as_slice()).expect("failed to load database");
            self.holder = Normal(database)
        } else {
            panic!("pre_update must be called for low_memory database updates")
        }
        debug!("reloading objects");
        for (id, sdo) in self.sdos.iter_mut() {
            let object = if let LowMemory(database) = &self.holder {
                database.get_object(*id).expect("invalid database")
            } else if let Normal(database) = &self.holder {
                database.get_object(*id).unwrap().clone()
            } else {
                panic!("cannot request objects while the database is being updated");
            };

            let mut obj = sdo.object.lock().unwrap();
            *obj = object;
            sdo.changed.store(true, Ordering::SeqCst)
        }
    }

    pub fn get_object<I: ObjectImpl>(&mut self, id: u64) -> Option<Arc<SystemDatabaseObject>> {
        if self.sdos.contains_key(&id) {
            return Some(self.sdos[&id].clone());
        }
        let object = if let LowMemory(database) = &self.holder {
            database.get_object(id).ok()
        } else if let Normal(database) = &self.holder {
            database.get_object(id).cloned()
        } else {
            panic!("cannot request objects while the database is being updated");
        };
        let Some(object) = object else {
            return None
        };
        let parsed = I::from_object(object)?;
        let sdo = SystemDatabaseObject {
            changed: AtomicBool::new(false),
            object: Mutex::new(parsed.to_object()),
        };
        self.sdos.insert(id, Arc::new(sdo));
        Some(self.sdos[&id].clone())
    }
}

pub struct SystemDatabaseObject {
    object: Mutex<Object>,
    changed: AtomicBool,
}

impl SystemDatabaseObject {
    pub fn object(&self) -> &Mutex<Object> {
        self.changed.store(false, Ordering::SeqCst);
        &self.object
    }
    pub fn has_changed(&self) -> bool {
        self.changed.load(Ordering::SeqCst)
    }
}
