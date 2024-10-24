use std::fs::File;
use std::io::{Error, Read, Seek, SeekFrom, Write};
use std::path::Path;
use crate::utils::file_utils;

/**
We write the structs with `bincode::encode` to a file.
To access each entry we need a index file where we can find the
Entries with offset and size of the encoded struct.
This is used for the index file where first entry is the index
of the encoded file, and size is the size of the encoded struct.

We also use it for different purposes, like storing id -> to id mapping.
 */
pub(in crate::repository) struct IndexRecord {
    pub left: u32,
    pub right: u32,
}

impl IndexRecord {
    pub fn from_file(file: &mut File, offset: u32) -> Result<Self, Error> {
        file.seek(SeekFrom::Start(u64::from(offset)))?;
        let mut left_bytes = [0u8; 4];
        let mut right_bytes = [0u8; 4];
        file.read_exact(&mut left_bytes)?;
        file.read_exact(&mut right_bytes)?;
        let left = u32::from_le_bytes(left_bytes);
        let right = u32::from_le_bytes(right_bytes);
        Ok(IndexRecord { left, right })
    }

    pub fn to_file(path: &Path, left: u32, right: u32, append: bool) -> Result<(), Error> {
        match file_utils::open_file_append(path, append) {
            Ok(mut file) => {
                let bytes = IndexRecord::to_bytes(left, right);
                file.write_all(&bytes)
            },
            Err(err) => Err(err)
        }
    }

    pub fn to_bytes(left: u32, right: u32) -> [u8; 8] {
        let left_bytes: [u8; 4] = left.to_le_bytes();
        let right_bytes: [u8; 4] = right.to_le_bytes();
        let mut combined_bytes: [u8; 8] = [0; 8];
        combined_bytes[..4].copy_from_slice(&left_bytes);
        combined_bytes[4..].copy_from_slice(&right_bytes);
        combined_bytes
    }

    pub fn get_record_size() -> u32 { 8 }
    pub fn get_index_offset(index: u32) -> u32 { index  * 8 }
}