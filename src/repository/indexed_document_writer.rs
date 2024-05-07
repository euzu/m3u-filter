use std::fs::File;
use std::io::{Error, ErrorKind, Write};
use std::path::PathBuf;

use crate::repository::index_record::IndexRecord;
use crate::utils::file_utils;
use crate::utils::file_utils::create_file_tuple;

pub(crate) struct IndexedDocumentWriter {
    main_path: PathBuf,
    index_path: PathBuf,
    main_file: File,
    index_file: File,
    index_offset: u32,
}

impl IndexedDocumentWriter {
    pub fn new(main_path: PathBuf, index_path: PathBuf) -> Result<Self, Error> {
        match create_file_tuple(&main_path, &index_path) {
            Ok((main_file, index_file)) => {
                Ok(IndexedDocumentWriter {
                    main_path,
                    index_path,
                    main_file,
                    index_file,
                    index_offset: 0,
                })
            }
            Err(e) => Err(e)
        }
    }
    pub fn write_doc<T>(&mut self, document_id: &mut u32, doc: &T) -> Result<(), Error>
        where
            T: ?Sized + serde::Serialize {
        if let Ok(encoded) = bincode::serialize(doc) {
            match file_utils::check_write(self.main_file.write_all(&encoded)) {
                Ok(_) => {
                    let bytes_written = encoded.len() as u16;
                    let combined_bytes = IndexRecord::to_bytes(self.index_offset, bytes_written);
                    if let Err(err) = file_utils::check_write(self.index_file.write_all(&combined_bytes)) {
                        return Err(Error::new(ErrorKind::Other, format!("failed to write document: {} - {}", self.index_path.to_str().unwrap(), err)));
                    }
                    self.index_offset += bytes_written as u32;
                    *document_id += 1;
                }
                Err(err) => {
                    return Err(Error::new(ErrorKind::Other, format!("failed to write document: {} - {}", self.main_path.to_str().unwrap(), err)));
                }
            }
        }
        Ok(())
    }
}