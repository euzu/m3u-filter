use std::fs::File;
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::Path;
use crate::repository::bplustree::BPlusTreeQuery;

pub(in crate::repository) type OffsetPointer = u32;

pub(in crate::repository) struct IndexedDocument {}

impl IndexedDocument {
    pub(in crate::repository) fn read_fragmentation(file: &mut File) -> std::io::Result<bool> {
        file.seek(SeekFrom::Start(0))?;
        let mut bool_bytes = [0u8];
        file.read_exact(&mut bool_bytes)?;
        Ok(u8::from_le_bytes(bool_bytes) == 1)
    }

    pub(in crate::repository) fn write_fragmentation(file: &mut File, fragmented: bool) -> std::io::Result<()> {
        file.seek(SeekFrom::Start(0))?;
        let fragmented_byte = if fragmented { 1u8.to_le_bytes() } else { 0u8.to_le_bytes() };
        file.write_all(&fragmented_byte)
    }

    pub(in crate::repository) fn read_content_size(main_file: &mut File) -> Result<usize, Error> {
        let mut size_bytes = [0u8; 4];
        main_file.read_exact(&mut size_bytes)?;
        let buf_size = u32::from_le_bytes(size_bytes) as usize;
        Ok(buf_size)
    }


    pub(in crate::repository) fn get_offset(index_path: &Path, doc_id: u32) -> Result<u64, Error> {
        match BPlusTreeQuery::<u32, OffsetPointer>::try_new(index_path) {
            Ok(mut tree) => {
                match tree.query(&doc_id) {
                    Some(offset) => Ok(u64::from(offset)),
                    None => Err(Error::new(ErrorKind::NotFound, format!("doc_id not found {doc_id}"))),
                }
            }
            Err(err) => Err(err)
        }
    }
}