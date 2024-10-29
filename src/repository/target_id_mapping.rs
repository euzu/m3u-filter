use std::cmp::max;
use std::collections::BTreeMap;
use std::io::Error;
use std::path::{Path, PathBuf};

use chrono::Local;
use log::error;
use serde::{Deserialize, Serialize};

use crate::model::playlist::PlaylistItemType;
use crate::repository::bplustree::BPlusTree;

const EXPIRATION_DURATION: i64 = 86400;

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct VirtualIdRecord {
    pub virtual_id: u32,
    pub provider_id: u32,
    pub uuid: [u8; 32],
    pub item_type: PlaylistItemType,
    pub parent_virtual_id: u32, // only for series to hold series info id.
    pub last_updated: i64,
}

impl VirtualIdRecord {
    fn new(provider_id: u32, virtual_id: u32, item_type: PlaylistItemType, parent_virtual_id: u32, uuid: [u8; 32]) -> Self {
        let last_updated = Local::now().timestamp();
        VirtualIdRecord { provider_id, virtual_id, uuid, item_type, parent_virtual_id, last_updated }
    }

    pub(crate) fn is_expired(&self) -> bool {
        (Local::now().timestamp() - self.last_updated) > EXPIRATION_DURATION
    }

    pub(crate) fn copy_update_timestamp(&self) -> Self {
        Self::new(self.provider_id, self.virtual_id, self.item_type, self.parent_virtual_id, self.uuid)
    }
}

pub(crate) struct TargetIdMapping {
    dirty: bool,
    virtual_id_counter: u32,
    by_virtual_id: BPlusTree<u32, VirtualIdRecord>,
    by_uuid: BTreeMap<[u8; 32], u32>,
    path: PathBuf,
}

impl TargetIdMapping {
    pub(crate) fn new(path: &Path) -> Self {
        let tree_virtual_id: BPlusTree<u32, VirtualIdRecord> = match BPlusTree::<u32, VirtualIdRecord>::deserialize(&path) {
            Ok(tree) => tree,
            _ => BPlusTree::<u32, VirtualIdRecord>::new()
        };
        let mut tree_uuid = BTreeMap::new();
        let mut virtual_id_counter: u32 = 0;
        tree_virtual_id.traverse(|keys, values| {
            match keys.iter().max() {
                None => {}
                Some(max_value) => {
                    virtual_id_counter = max(virtual_id_counter, *max_value);
                }
            }
            values.iter().for_each(|v| {
                tree_uuid.insert(v.uuid, v.virtual_id);
            });
        });
        TargetIdMapping {
            dirty: false,
            virtual_id_counter,
            by_virtual_id: tree_virtual_id,
            by_uuid: tree_uuid,
            path: path.to_path_buf(),
        }
    }

    pub(crate) fn insert_entry(&mut self, uuid: [u8; 32], provider_id: u32, item_type: &PlaylistItemType, parent_virtual_id: u32) -> u32 {
        match self.by_uuid.get(&uuid) {
            None => {
                self.dirty = true;
                self.virtual_id_counter += 1;
                let record = VirtualIdRecord::new(provider_id, self.virtual_id_counter, *item_type, parent_virtual_id, uuid);
                self.by_virtual_id.insert(self.virtual_id_counter, record);
                self.virtual_id_counter
            }
            Some(record) => *record
        }
    }

    pub(crate) fn persist(&mut self) -> Result<(), Error> {
        if self.dirty {
            self.by_virtual_id.serialize(&self.path)?;
        }
        self.dirty = false;
        Ok(())
    }
}

impl Drop for TargetIdMapping {
    fn drop(&mut self) {
        match self.persist() {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to persist target id mapping {:?} err:{}", &self.path, err.to_string())
            }
        }
    }
}