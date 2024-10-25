use std::convert::TryFrom;
use std::fs::File;
use std::io::{Error, ErrorKind, Write};
use std::path::PathBuf;

use crate::utils::file_utils;
use crate::utils::file_utils::create_file_tuple;

pub(in crate::repository) fn get_record_size() -> u32 { 8 }

fn to_bytes(left: u32, right: u32) -> [u8; 8] {
    let left_bytes: [u8; 4] = left.to_le_bytes();
    let right_bytes: [u8; 4] = right.to_le_bytes();
    let mut combined_bytes: [u8; 8] = [0; 8];
    combined_bytes[..4].copy_from_slice(&left_bytes);
    combined_bytes[4..].copy_from_slice(&right_bytes);
    combined_bytes
}

/**
* Creates two files,
* - content
* - index
*
* Layout of content file record is:
*   - content-size (u32) + content
*
* Layout of index file record is:
*  - document_id (u32) + content file offset + (u32)
*/
pub(in crate::repository) struct IndexedDocumentWriter {
    main_path: PathBuf,
    index_path: PathBuf,
    main_file: File,
    index_file: File,
    main_offset: u32,
    index_offset: u32,
}

impl IndexedDocumentWriter {
    fn new_with_mode(main_path: PathBuf, index_path: PathBuf, append: bool) -> Result<Self, Error> {
        match create_file_tuple(&main_path, &index_path, append) {
            Ok((main_file, index_file)) => {
                let main_offset = match &main_file.metadata() {
                    Ok(meta) => u32::try_from(meta.len()).map_err(|err| Error::new(ErrorKind::Other, err))?,
                    Err(_) => 0
                };
                let index_offset = match &index_file.metadata() {
                    Ok(meta) => u32::try_from(meta.len()).map_err(|err| Error::new(ErrorKind::Other, err))?,
                    Err(_) => 0
                };
                Ok(Self {
                    main_path,
                    index_path,
                    main_file,
                    index_file,
                    main_offset,
                    index_offset,
                })
            }
            Err(e) => Err(e)
        }
    }

    pub fn new(main_path: PathBuf, index_path: PathBuf) -> Result<Self, Error> {
        Self::new_with_mode(main_path, index_path, false)
    }

    pub fn new_append(main_path: PathBuf, index_path: PathBuf) -> Result<Self, Error> {
        Self::new_with_mode(main_path, index_path, true)
    }

    pub fn write_doc<T>(&mut self, doc_id: u32, doc: &T) -> Result<(u32, u32), Error>
        where
            T: ?Sized + serde::Serialize {
        let current_main_index = self.main_offset;
        let current_index_index = self.index_offset;
        if let Ok(encoded) = bincode::serialize(doc) {
            let content_bytes = u32::try_from(encoded.len()).map_err(|err| Error::new(ErrorKind::Other, err))?;
            let mut data: Vec<u8> = content_bytes.to_le_bytes().to_vec();
            data.extend(&encoded);
            match file_utils::check_write(&self.main_file.write_all(&data)) {
                Ok(()) => {
                    let combined_bytes = to_bytes(doc_id, self.main_offset);
                    if let Err(err) = file_utils::check_write(&self.index_file.write_all(&combined_bytes)) {
                        return Err(Error::new(ErrorKind::Other, format!("failed to write document: {} - {}", self.index_path.to_str().unwrap(), err)));
                    }
                    let written_bytes = u32::try_from(data.len()).map_err(|err| Error::new(ErrorKind::Other, err))?;
                    self.main_offset += written_bytes;
                    self.index_offset += get_record_size();
                }
                Err(err) => {
                    return Err(Error::new(ErrorKind::Other, format!("failed to write document: {} - {}", self.main_path.to_str().unwrap(), err)));
                }
            }
        }
        Ok((current_main_index, current_index_index))
    }
}