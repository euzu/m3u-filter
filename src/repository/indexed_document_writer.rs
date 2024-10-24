use std::convert::TryFrom;
use std::fs::File;
use std::io::{Error, ErrorKind, Write};
use std::path::PathBuf;

use crate::repository::index_record::IndexRecord;
use crate::utils::file_utils;
use crate::utils::file_utils::create_file_tuple;

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
            match file_utils::check_write(&self.main_file.write_all(&encoded)) {
                Ok(()) => {
                    let bytes_written = u32::try_from(encoded.len()).map_err(|err| Error::new(ErrorKind::Other, err))?;
                    let combined_bytes = IndexRecord::to_bytes(doc_id, bytes_written);
                    if let Err(err) = file_utils::check_write(&self.index_file.write_all(&combined_bytes)) {
                        return Err(Error::new(ErrorKind::Other, format!("failed to write document: {} - {}", self.index_path.to_str().unwrap(), err)));
                    }
                    self.main_offset += bytes_written;
                    self.index_offset += IndexRecord::get_record_size();
                }
                Err(err) => {
                    return Err(Error::new(ErrorKind::Other, format!("failed to write document: {} - {}", self.main_path.to_str().unwrap(), err)));
                }
            }
        }
        Ok((current_main_index, current_index_index))
    }
}