use std::cmp::max;
use std::collections::BTreeMap;
use std::io::Error;
use std::path::{Path, PathBuf};

use chrono::Local;
use log::error;
use serde::{Deserialize, Serialize};

use crate::model::playlist::{PlaylistItemType, UUIDType};
use crate::repository::bplustree::BPlusTree;

// TODO make configurable
const EXPIRATION_DURATION: i64 = 86400;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VirtualIdRecord {
    pub virtual_id: u32,
    pub provider_id: u32,
    pub uuid: UUIDType,
    pub item_type: PlaylistItemType,
    pub parent_virtual_id: u32, // only for series to hold series info id.
    pub last_updated: i64,
}

impl VirtualIdRecord {
    fn new(provider_id: u32, virtual_id: u32, item_type: PlaylistItemType, parent_virtual_id: u32, uuid: UUIDType) -> Self {
        let last_updated = Local::now().timestamp();
        Self { virtual_id, provider_id, uuid, item_type, parent_virtual_id, last_updated }
    }

    pub fn is_expired(&self) -> bool {
        (Local::now().timestamp() - self.last_updated) > EXPIRATION_DURATION
    }

    pub fn copy_update_timestamp(&self) -> Self {
        Self::new(self.provider_id, self.virtual_id, self.item_type, self.parent_virtual_id, self.uuid)
    }
}

pub struct TargetIdMapping {
    dirty: bool,
    virtual_id_counter: u32,
    by_virtual_id: BPlusTree<u32, VirtualIdRecord>,
    by_uuid: BTreeMap<UUIDType, u32>,
    path: PathBuf,
}

impl TargetIdMapping {
    pub fn new(path: &Path) -> Self {
        let tree_virtual_id: BPlusTree<u32, VirtualIdRecord> = BPlusTree::<u32, VirtualIdRecord>::load(path).unwrap_or_else(|_| BPlusTree::<u32, VirtualIdRecord>::new());
        let mut tree_uuid = BTreeMap::new();
        let mut virtual_id_counter: u32 = 0;
        tree_virtual_id.traverse(|keys, values| {
            match keys.iter().max() {
                None => {}
                Some(max_value) => {
                    virtual_id_counter = max(virtual_id_counter, *max_value);
                }
            }
            for v in values {
                tree_uuid.insert(v.uuid, v.virtual_id);
            }
        });
        Self {
            dirty: false,
            virtual_id_counter,
            by_virtual_id: tree_virtual_id,
            by_uuid: tree_uuid,
            path: path.to_path_buf(),
        }
    }

    // pub fn get_virtual_id(&mut self, uuid: UUIDType, provider_id: u32, item_type: PlaylistItemType, parent_virtual_id: u32) -> u32 {
    //     match self.by_uuid.get(&uuid) {
    //         None => {
    //             self.dirty = true;
    //             self.virtual_id_counter += 1;
    //             let record = VirtualIdRecord::new(provider_id, self.virtual_id_counter, item_type, parent_virtual_id, uuid);
    //             self.by_virtual_id.insert(self.virtual_id_counter, record);
    //             self.virtual_id_counter
    //         }
    //         Some(virtual_id) => *virtual_id
    //     }
    // }

    pub fn get_and_update_virtual_id(&mut self, uuid: UUIDType, provider_id: u32, item_type: PlaylistItemType, parent_virtual_id: u32) -> u32 {
        match self.by_uuid.get(&uuid) {
            None => {
                self.dirty = true;
                self.virtual_id_counter += 1;
                let virtual_id = self.virtual_id_counter;
                let record = VirtualIdRecord::new(provider_id, virtual_id, item_type, parent_virtual_id, uuid);
                self.by_virtual_id.insert(virtual_id, record);
                self.virtual_id_counter
            }
            Some(virtual_id) => {
                if let Some(record) = self.by_virtual_id.query(virtual_id) {
                    if record.provider_id == provider_id && (record.item_type != item_type || record.parent_virtual_id != parent_virtual_id) {
                        let new_record = VirtualIdRecord::new(provider_id, *virtual_id, item_type, parent_virtual_id, uuid);
                        println!("updating record {virtual_id} {record:?} {new_record:?} ");
                        self.by_virtual_id.insert(*virtual_id, new_record);
                        self.dirty = true;
                    }
                }
                *virtual_id
            }
        }
    }

    pub fn persist(&mut self) -> Result<(), Error> {
        if self.dirty {
            self.by_virtual_id.store(&self.path)?;
        }
        self.dirty = false;
        Ok(())
    }
}

impl Drop for TargetIdMapping {
    fn drop(&mut self) {
        if let Err(err) = self.persist() {
            error!("Failed to persist target id mapping {:?} err:{err}", &self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use crate::repository::bplustree::BPlusTree;
    use crate::repository::target_id_mapping::{VirtualIdRecord};

    #[test]
    fn test_id_mapping() {
        let path = PathBuf::from("../m3u-test/settings/m3u-silver/data/xt_m3u/id_mapping.db");
        let mapping = BPlusTree::<u32, VirtualIdRecord>::load(&path);
        mapping.unwrap().traverse(|keys, values| {
            for (key, value) in keys.iter().zip(values.iter()) {
                println!("{key:?} {value:?}\n");
            }
        });
    }
}
