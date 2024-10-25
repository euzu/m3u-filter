use std::cmp::max;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::rc::Rc;
use actix_web::body::MessageBody;
use crate::model::playlist::{PlaylistItemType};

use crate::utils::file_utils;

/**
This file contains the provider id, the virtual id, and an uuid

 */
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
    by_provider_id: BTreeMap<u32, Rc<TargetIdMappingRecord>>,
    by_virtual_id: BTreeMap<u32, Rc<TargetIdMappingRecord>>,
    by_uuid: BTreeMap<[u8; 32], Rc<TargetIdMappingRecord>>,
    records: Vec<Rc<TargetIdMappingRecord>>,
    file: Option<File>
}

impl TargetIdMapping {
    pub(crate) fn insert_entry(&mut self, provider_id: u32, uuid: [u8; 32], item_type: &PlaylistItemType, parent_virtual_id: u32) -> u32 {
        self.dirty = true;
        self.virtual_id_counter += 1;
        self.insert(TargetIdMappingRecord { provider_id, virtual_id: self.virtual_id_counter, uuid, item_type: *item_type, parent_virtual_id});
        self.virtual_id_counter
    }

    fn new(records: Vec<Rc<TargetIdMappingRecord>>, file: Option<File>) -> Self {
        let mut by_provider_id: BTreeMap<u32, Rc<TargetIdMappingRecord>> = BTreeMap::new();
        let mut by_virtual_id: BTreeMap<u32, Rc<TargetIdMappingRecord>> = BTreeMap::new();
        let mut by_uuid: BTreeMap<[u8; 32], Rc<TargetIdMappingRecord>> = BTreeMap::new();
        let mut virtual_id_counter: u32 = 0;
        for record in &records {
            by_provider_id.insert(record.provider_id, Rc::clone(record));
            by_virtual_id.insert(record.virtual_id, Rc::clone(record));
            by_uuid.insert(record.uuid, Rc::clone(record));
            virtual_id_counter = max(record.virtual_id, virtual_id_counter);
        }
        TargetIdMapping {
            dirty: false,
            virtual_id_counter,
            by_provider_id,
            by_virtual_id,
            by_uuid,
            records,
            file
        }
    }

    fn insert(&mut self, record: TargetIdMappingRecord) {
        let provider_id = record.provider_id;
        let virtual_id = record.virtual_id;
        let uuid = record.uuid;
        let shared_record = Rc::new(record);
        self.by_provider_id.insert(provider_id, Rc::clone(&shared_record));
        self.by_virtual_id.insert(virtual_id, Rc::clone(&shared_record));
        self.by_uuid.insert(uuid, Rc::clone(&shared_record));
        self.records.push(shared_record);
    }

   pub(crate) fn get_by_provider_id(&self, provider_id: u32) -> Option<&Rc<TargetIdMappingRecord>> {
        self.by_provider_id.get(&provider_id)
    }

    pub(crate) fn get_by_virtual_id(&self, virtual_id: u32) -> Option<&Rc<TargetIdMappingRecord>> {
        self.by_virtual_id.get(&virtual_id)
    }

    pub(crate) fn get_by_uuid(&self, uuid: &[u8; 32]) -> Option<&Rc<TargetIdMappingRecord>> {
        self.by_uuid.get(uuid)
    }


    pub fn persist(&mut self) -> Result<(), Error> {
        let mut file = match self.file.take() {
            Some(file) => file,
            None => return Err(Error::new(ErrorKind::NotFound, "No file given")),
        };
        let result = self.to_file(&mut file);
        self.file = Some(file);
        result
    }

    pub fn to_file(&mut self, file: &mut File) -> Result<(), Error> {
        if self.dirty {
            for record in &self.records {
                let bytes = record.to_bytes();
                if let Err(err) = file.write_all(&bytes) {
                    return Err(err);
                }
            }
        }
        self.dirty = false;
        Ok(())
    }

    pub fn to_path(&mut self, path: &Path) -> Result<(), Error>  {
        match file_utils::open_file_append(path, false) {
            Ok(mut file) => self.to_file(&mut file),
            Err(err) => Err(err)
        }
    }

    pub fn from_path(path: &Path) -> Self {
        match file_utils::open_file_append(path, false) {
            Ok(mut file) => {
                let records = TargetIdMapping::read_records_from_file(&mut file);
                TargetIdMapping::new(records, Some(file))
            },
            _ => TargetIdMapping::new(vec![], None)
        }
    }

    fn read_records_from_file(file: &mut File) -> Vec<Rc<TargetIdMappingRecord>> {
        let mut records = vec![];
        if let Ok(_) = file.seek(SeekFrom::Start(0)) {
            let mut bytes = [0u8;45];
            loop {
                if let Err(_) = file.read_exact(&mut bytes) {
                    break;
                }
                records.push(Rc::new(TargetIdMappingRecord::from_bytes(&bytes)));
            }
        }
        records
    }
}