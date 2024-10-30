use std::convert::TryFrom;
use std::fs::File;
use std::io::{Error, ErrorKind, Write};
use std::path::PathBuf;
use log::error;
use crate::repository::bplustree::BPlusTree;

use crate::utils::file_utils;
use crate::utils::file_utils::{open_file_append};

pub(in crate::repository) type OffsetPointer = u32;

/**
* Creates two files,
* - content
* - index
*
* Layout of content file record is:
*   - content-size (u32) + content (deflate)
*
* index file is a bplustree
*/
pub(in crate::repository) struct IndexedDocumentWriter {
    main_path: PathBuf,
    index_path: PathBuf,
    main_file: File,
    main_offset: OffsetPointer,
    index_tree: BPlusTree<u32, OffsetPointer>,
}

impl IndexedDocumentWriter {
    fn new_with_mode(main_path: PathBuf, index_path: PathBuf, append: bool) -> Result<Self, Error> {
        // Attempt to open the main file in the specified mode (append or not)
        let main_file = open_file_append(&main_path, append)?;

        // Retrieve file size and convert to `u32` for `main_offset`, if possible
        let main_offset = main_file
            .metadata()
            .and_then(|meta| u32::try_from(meta.len()).map_err(|err| Error::new(ErrorKind::Other, err)))
            .unwrap_or(0);

        // Initialize the index tree (BPlusTree) - either by deserializing an existing one or creating a new one
        let index_tree = if append && index_path.exists() {
            BPlusTree::<u32, OffsetPointer>::deserialize(&index_path).unwrap_or_else(|err| {
                error!("Failed to load index {:?}: {}", index_path, err);
                BPlusTree::<u32, OffsetPointer>::new()
            })
        } else {
            BPlusTree::<u32, OffsetPointer>::new()
        };

        Ok(Self {
            main_path,
            index_path,
            main_file,
            main_offset,
            index_tree,
        })
    }

    pub fn new(main_path: PathBuf, index_path: PathBuf) -> Result<Self, Error> {
        Self::new_with_mode(main_path, index_path, false)
    }

    pub fn new_append(main_path: PathBuf, index_path: PathBuf) -> Result<Self, Error> {
        Self::new_with_mode(main_path, index_path, true)
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.main_file.flush()?;
        self.index_tree.serialize(&self.index_path).map(|_| ())
    }

    pub fn write_doc<T>(&mut self, doc_id: u32, doc: &T) -> Result<(), Error>
        where
            T: ?Sized + serde::Serialize {
        let encoded_bytes = bincode::serialize(doc).map_err(|_| Error::new(ErrorKind::InvalidData, "Failed to deserialize document"))?;
        let encoded_bytes_len =  u32::try_from(encoded_bytes.len()).map_err(|err| Error::new(ErrorKind::Other, err))?;
        self.main_file.write_all(&encoded_bytes_len.to_le_bytes())?;
        match file_utils::check_write(&self.main_file.write_all(&encoded_bytes)) {
            Ok(()) => {
                self.index_tree.insert(doc_id, self.main_offset);
                let written_bytes = u32::try_from(encoded_bytes.len() + 4).map_err(|err| Error::new(ErrorKind::Other, err))?;
                self.main_offset += written_bytes;
            }
            Err(err) => {
                return Err(Error::new(ErrorKind::Other, format!("failed to write document: {} - {}", self.main_path.to_str().unwrap(), err)));
            }
        }
        Ok(())
    }
}