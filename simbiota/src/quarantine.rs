use crate::daemon_config::DaemonConfig;
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions, Permissions};
use std::io::{Read, Write};

use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::os::linux::fs::MetadataExt;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct QuarantineEntryInfo {
    pub original_path: String,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct LegacyQuarantineEntryInfo {
    pub original_path: OsString,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
}

impl QuarantineEntryInfo {
    pub fn serialize(&self) -> String {
        serde_json::to_string(self).expect("failed to serialize quarantine entry")
    }

    pub fn deserialize(json: &str) -> Self {
        serde_json::from_str(json).expect("failed to deserialize quarantine entry")
    }
}

struct QuaratineEntry {
    id: String,
    info: QuarantineEntryInfo,
}

pub(crate) struct Quarantine {
    quarantine_dir: PathBuf,
}

impl Quarantine {
    pub fn new(daemon_config: Arc<DaemonConfig>) -> Self {
        let dir_path = &daemon_config.quarantine.path;
        if !dir_path.exists() {
            std::fs::create_dir_all(dir_path).expect("failed to create quarantine directory");
        }
        std::fs::set_permissions(dir_path, Permissions::from_mode(0o0700))
            .expect("failed to set quarantine directory permissions");
        Self {
            quarantine_dir: daemon_config.quarantine.path.clone(),
        }
    }

    fn get_stored_entries(&self) -> Vec<QuaratineEntry> {
        let mut entries = Vec::new();
        for entry in
            std::fs::read_dir(&self.quarantine_dir).expect("failed to read quarantine directory")
        {
            let entry = entry.expect("failed to read quarantine dir entry");
            let filename = entry.file_name().to_string_lossy().to_string();
            if filename.starts_with('.') && filename.ends_with(".info") {
                // check whether the file is present for the given info

                if let Some(f) = filename
                    .strip_prefix('.')
                    .and_then(|f| f.strip_suffix(".info"))
                {
                    let entry_path = self.quarantine_dir.join(f);
                    if !entry_path.exists() {
                        warn!(
                            "quarantine entry for entry info does not exists, removing: {filename}"
                        );
                        if std::fs::remove_file(entry.path()).is_err() {
                            error!(
                                "failed to remove quarantine entry info: {}",
                                entry.path().display()
                            );
                        }
                        continue;
                    }
                }
            } else {
                // this is a normal entry
                let info_name = format!(".{}.info", filename);
                let info_path = self.quarantine_dir.join(&info_name);

                if !info_path.exists() {
                    warn!("quarantine entry info for entry does not exists, removing: {filename}");
                    if std::fs::remove_file(entry.path()).is_err() {
                        error!(
                            "failed to remove quarantine entry: {}",
                            entry.path().display()
                        );
                    }
                    continue;
                }

                // check for older version of entries with OsString and convert them to String
                // this is neccessary because we switched from OsString to String after we release the first version
                let contents = std::fs::read_to_string(&info_path)
                    .expect("failed to read quarantine entry info");
                let json_data = serde_json::from_str::<LegacyQuarantineEntryInfo>(&contents);
                if let Ok(legacy_info) = json_data {
                    warn!("converting legacy quarantine entry info to new format: {filename}");
                    let info = QuarantineEntryInfo {
                        original_path: legacy_info.original_path.to_string_lossy().to_string(),
                        uid: legacy_info.uid,
                        gid: legacy_info.gid,
                        mode: legacy_info.mode,
                    };
                    std::fs::write(&info_path, info.serialize())
                        .expect("failed to write quarantine entry info");
                }

                match std::fs::read_to_string(&info_path) {
                    Ok(info_data) => {
                        let info = QuarantineEntryInfo::deserialize(&info_data);
                        entries.push(QuaratineEntry { id: filename, info });
                    }
                    Err(e) => {
                        error!(
                            "failed to read quarantine entry info: {}, removing file ({e:?})",
                            info_path.display()
                        );
                        std::fs::remove_file(info_path)
                            .unwrap_or_else(|e| error!("failed to remove info file: {:?}", e));
                    }
                }
            }
        }

        entries
    }

    pub fn get_entries(&self) -> Vec<QuarantineEntryInfo> {
        self.get_stored_entries()
            .iter()
            .map(|e| e.info.clone())
            .collect()
    }

    pub fn get_entry_by_id(&self, id: usize) -> Option<QuarantineEntryInfo> {
        self.get_stored_entries().get(id).map(|e| e.info.clone())
    }

    pub fn get_entry_by_name(&self, name: &str) -> Option<QuarantineEntryInfo> {
        self.get_stored_entries()
            .iter()
            .find(|entry| entry.info.original_path.as_str() == name)
            .map(|e| e.info.clone())
    }

    pub fn remove_entry(&mut self, entry: QuarantineEntryInfo) {
        let entries = self.get_stored_entries();
        let e = entries.iter().find(|e| e.info == entry);
        if let Some(entry) = e {
            std::fs::remove_file(self.quarantine_dir.join(&entry.id))
                .expect("failed to remove quarantine entry");
            std::fs::remove_file(self.quarantine_dir.join(format!(".{}.info", &entry.id)))
                .expect("failed to remove quarantine entry info");
            self.get_stored_entries();
        }
    }

    pub fn restore_entry(&mut self, entry: QuarantineEntryInfo) {
        let entries = self.get_stored_entries();
        let e = entries.iter().find(|e| e.info == entry);
        if let Some(entry) = e {
            std::fs::rename(
                self.quarantine_dir.join(&entry.id),
                &entry.info.original_path,
            )
            .expect("failed to remove quarantine entry");
            std::fs::set_permissions(
                &entry.info.original_path,
                Permissions::from_mode(entry.info.mode),
            )
            .expect("failed to set file permissions");
            std::fs::remove_file(self.quarantine_dir.join(format!(".{}.info", &entry.id)))
                .expect("failed to remove quarantine entry info");
            self.get_stored_entries();
        }
    }

    pub fn add_file(&mut self, filename: &str) {
        warn!("moving file to quarantine: {filename}");
        let original_path = Path::new(filename);
        if !original_path.exists() {
            warn!("file added to quarantine but it does not exists");
            return;
        }
        let meta = original_path
            .metadata()
            .expect("failed to get file metadata");

        let quarantine_entry = QuarantineEntryInfo {
            original_path: original_path
                .to_path_buf()
                .into_os_string()
                .into_string()
                .expect("somehow ended up with a non-utf8 filename"),
            mode: meta.st_mode(),
            uid: meta.st_uid(),
            gid: meta.st_gid(),
        };

        let entry_id = uuid::Uuid::new_v4();
        let mut entry_path = self.quarantine_dir.clone();
        entry_path.push(entry_id.to_string());

        // move file to quarantine
        std::fs::rename(original_path, &entry_path).expect("failed to move file to quarantine");
        std::fs::set_permissions(&entry_path, Permissions::from_mode(0o0000))
            .expect("failed to set quarantine file permissions");
        // store entry info alongside the file
        let info_entry_path = self.quarantine_dir.join(format!(".{}.info", entry_id));
        std::fs::write(&info_entry_path, quarantine_entry.serialize())
            .expect("failed to write quarantine entry info");
        std::fs::set_permissions(&info_entry_path, Permissions::from_mode(0o0600))
            .expect("failed to set quarantine file permissions");
    }
}
