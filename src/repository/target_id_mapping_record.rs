use std::cmp::max;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::playlist::PlaylistItemType;
use crate::repository::bplustree::BPlusTree;
use crate::utils::file_utils;

/**
This file contains the provider id, the virtual id, and an uuid
 */
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct TargetIdMappingRecord {
    pub provider_id: u32,
    pub virtual_id: u32,
    pub uuid: [u8; 32],
    pub item_type: PlaylistItemType,
    pub parent_virtual_id: u32, // only for series to hold series info id.
}

impl TargetIdMappingRecord {
    fn to_bytes(&self) -> [u8; 45] {
        let provider_id_bytes: [u8; 4] = self.provider_id.to_le_bytes();
        let virtual_id_bytes: [u8; 4] = self.virtual_id.to_le_bytes();
        let parent_virtual_id_bytes: [u8; 4] = self.parent_virtual_id.to_le_bytes();
        let item_type_bytes: [u8; 1] = self.item_type.to_bytes();

        let mut combined_bytes: [u8; 45] = [0; 45];
        combined_bytes[0..4].copy_from_slice(&provider_id_bytes);
        combined_bytes[4..8].copy_from_slice(&virtual_id_bytes);
        combined_bytes[8..12].copy_from_slice(&parent_virtual_id_bytes);
        combined_bytes[12..13].copy_from_slice(&item_type_bytes);
        combined_bytes[13..45].copy_from_slice(&self.uuid);

        combined_bytes
    }

    fn from_bytes(bytes: &[u8; 45]) -> Self {
        let provider_id_bytes: [u8; 4] = bytes[0..4].try_into().expect("Slice with incorrect length");
        let virtual_id_bytes: [u8; 4] = bytes[4..8].try_into().expect("Slice with incorrect length");
        let parent_virtual_id_bytes: [u8; 4] = bytes[8..12].try_into().expect("Slice with incorrect length");
        let item_type_bytes: [u8; 1] = bytes[12..13].try_into().expect("Slice with incorrect length");
        let uuid: [u8; 32] = bytes[13..45].try_into().expect("Slice with incorrect length");

        TargetIdMappingRecord {
            provider_id: u32::from_le_bytes(provider_id_bytes),
            virtual_id: u32::from_le_bytes(virtual_id_bytes),
            parent_virtual_id: u32::from_le_bytes(parent_virtual_id_bytes),
            item_type: PlaylistItemType::from_bytes(item_type_bytes).unwrap(),
            uuid,
        }
    }
}

pub(crate) struct TargetIdMapping {
    dirty: bool,
    virtual_id_counter: u32,
    by_virtual_id: BPlusTree<u32, TargetIdMappingRecord>,
    path: PathBuf
}

impl TargetIdMapping {
    pub(crate) fn new(path: PathBuf) -> Self {
        let mut by_virtual_id: BPlusTree<u32, TargetIdMappingRecord> = match BPlusTree::<u32, TargetIdMappingRecord>::deserialize(&path) {
            Ok(tree) => {
                tree
            }
            _ => BPlusTree::<u32, TargetIdMappingRecord>::new()
        };

        let mut virtual_id_counter: u32 = 0;
        by_virtual_id.traverse(|node| {
            match node.max_key() {
                None => {}
                Some(max_value) => {
                    virtual_id_counter = max(virtual_id_counter, *max_value);
                }
            }
        });
        TargetIdMapping {
            dirty: false,
            virtual_id_counter,
            by_virtual_id,
            path
        }
    }

    fn insert(&mut self, record: TargetIdMappingRecord) {
        let virtual_id = record.virtual_id;
        self.by_virtual_id.insert(virtual_id, record);
    }

    pub(crate) fn insert_entry(&mut self, provider_id: u32, uuid: [u8; 32], item_type: &PlaylistItemType, parent_virtual_id: u32) -> u32 {
        self.dirty = true;
        self.virtual_id_counter += 1;
        self.insert(TargetIdMappingRecord { provider_id, virtual_id: self.virtual_id_counter, uuid, item_type: *item_type, parent_virtual_id });
        self.virtual_id_counter
    }

    pub(crate) fn get_by_virtual_id(&self, virtual_id: u32) -> Option<&TargetIdMappingRecord> {
        self.by_virtual_id.query(&virtual_id)
    }

    pub(crate) fn persist(&mut self) -> Result<(), Error> {
        if self.dirty {
            self.by_virtual_id.serialize(&self.path)?;
        }
        self.dirty = false;
        Ok(())
    }

}