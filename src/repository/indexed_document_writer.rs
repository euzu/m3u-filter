use std::convert::TryFrom;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Seek, SeekFrom, Write};
use std::path::PathBuf;

use log::error;

use crate::repository::bplustree::BPlusTree;
use crate::repository::indexed_document::{OffsetPointer, IndexedDocument};
use crate::utils::file_utils;

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
    dirty: bool,
    fragmented: bool,
}

impl IndexedDocumentWriter {
    fn new_with_mode(main_path: PathBuf, index_path: PathBuf, append: bool) -> Result<Self, Error> {
        let append_mode = append && main_path.exists();
        let mut main_file = if append_mode {
            OpenOptions::new()
                .read(true)
                .write(true)
                .truncate(false)
                .open(&main_path)
        } else {
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&main_path)
        }?;

        // Retrieve file size and convert to `u32` for `main_offset`, if possible
        let mut main_offset = main_file
            .metadata()
            .and_then(|meta| u32::try_from(meta.len()).map_err(|err| Error::new(ErrorKind::Other, err)))
            .unwrap_or(0);

        let mut fragmented = false;
        if main_offset == 0 {
            IndexedDocument::write_fragmentation(&mut main_file, false)?;
            main_offset = 1;
        } else {
            fragmented = IndexedDocument::read_fragmentation(&mut main_file)?;
        }

        // Initialize the index tree (BPlusTree) - either by deserializing an existing one or creating a new one
        let index_tree = if append_mode && index_path.exists() {
            BPlusTree::<u32, OffsetPointer>::load(&index_path).unwrap_or_else(|err| {
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
            dirty: false,
            fragmented,
        })
    }

    pub fn new(main_path: PathBuf, index_path: PathBuf) -> Result<Self, Error> {
        Self::new_with_mode(main_path, index_path, false)
    }

    pub fn new_append(main_path: PathBuf, index_path: PathBuf) -> Result<Self, Error> {
        Self::new_with_mode(main_path, index_path, true)
    }

    pub fn store(&mut self) -> std::io::Result<()> {
        if self.dirty {
            self.dirty = false;
            self.main_file.flush()?;
            self.index_tree.store(&self.index_path).map(|_| ())
        } else {
            Ok(())
        }
    }

    pub fn write_doc<T>(&mut self, doc_id: u32, doc: &T) -> Result<(), Error>
    where
        T: ?Sized + serde::Serialize,
    {
        self.dirty = true;
        let encoded_bytes = bincode::serialize(doc).map_err(|_| Error::new(ErrorKind::InvalidData, "Failed to serialize document"))?;

        let mut append = false;
        if let Some(&offset) = self.index_tree.query(&doc_id) {
            self.main_file.seek(SeekFrom::Start(u64::from(offset)))?;
            let size = IndexedDocument::read_content_size(&mut self.main_file)?;
            if encoded_bytes.len() > size {
                // does not fit we need to append, file is fragmented
                if !self.fragmented {
                    self.fragmented = true;
                    IndexedDocument::write_fragmentation(&mut self.main_file, true)?;
                }
                self.main_file.seek(SeekFrom::End(0))?;
                append = true;
            } else {
                self.main_file.seek(SeekFrom::Start(u64::from(offset)))?;
            }
        } else {
            self.main_file.seek(SeekFrom::End(0))?;
            append = true;
        }

        let encoded_bytes_len = u32::try_from(encoded_bytes.len()).map_err(|err| Error::new(ErrorKind::Other, err))?;
        self.main_file.write_all(&encoded_bytes_len.to_le_bytes())?;
        match file_utils::check_write(&self.main_file.write_all(&encoded_bytes)) {
            Ok(()) => {
                if append {
                    self.index_tree.insert(doc_id, self.main_offset);
                    let written_bytes = u32::try_from(encoded_bytes.len() + 4).map_err(|err| Error::new(ErrorKind::Other, err))?;
                    self.main_offset += written_bytes;
                }
            }
            Err(err) => {
                return Err(Error::new(ErrorKind::Other, format!("failed to write document: {} - {}", self.main_path.to_str().unwrap(), err)));
            }
        }
        Ok(())
    }
}

impl Drop for IndexedDocumentWriter {
    fn drop(&mut self) {
        let _ = self.store();
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use crate::repository::indexed_document_gc::IndexedDocumentGarbageCollector;
    use crate::repository::indexed_document_reader::IndexedDocumentReader;
    use crate::repository::indexed_document_writer::IndexedDocumentWriter;

    // Example usage with a simple struct
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    struct Record {
        id: u32,
        data: String,
    }

    #[test]
    fn insert_test() -> io::Result<()> {
        let main_path = PathBuf::from("/tmp/main.iw");
        let index_path = PathBuf::from("/tmp/main.iw.idx");
        {
            let mut idw = IndexedDocumentWriter::new(main_path.clone(), index_path.clone())?;

            for i in 0u32..=500 {
                idw.write_doc(i, &Record {
                    id: i,
                    data: format!("Entry {i}"),
                })?;
            }

            for i in 0u32..=500 {
                idw.write_doc(i, &Record {
                    id: i,
                    data: format!("E {}", i),
                })?;
            }


            // fragmentation
            for i in 0u32..=500 {
                idw.write_doc(i, &Record {
                    id: i,
                    data: format!("Entry {}", i + 9000),
                })?;
            }

            idw.store()?;
        }
        {
            let mut gc = IndexedDocumentGarbageCollector::new(main_path.clone(), index_path.clone())?;
            gc.garbage_collect()?;
        }
        {
            let reader = IndexedDocumentReader::<Record>::new(&main_path, &index_path)?;
            let mut i = 0;
            for doc in reader {
                assert_eq!(doc.id, i, "Wrong id");
                assert_eq!(doc.data, format!("Entry {}", i + 9000), "Wrong data");
                i += 1;
            }
            assert_eq!(501, i, "Wrong number of elements");
        }

        Ok(())
    }
}