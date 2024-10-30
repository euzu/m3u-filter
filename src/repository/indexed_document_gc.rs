use std::convert::TryFrom;
use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use crate::repository::bplustree::BPlusTree;
use crate::repository::indexed_document::{OffsetPointer, IndexedDocument};
use crate::utils::file_utils;

pub(in crate::repository) struct IndexedDocumentGarbageCollector {
    main_path: PathBuf,
    index_path: PathBuf,
    main_file: File,
    index_tree: BPlusTree<u32, OffsetPointer>,
}

impl IndexedDocumentGarbageCollector {
    pub fn new(main_path: PathBuf, index_path: PathBuf) -> Result<Self, Error> {
        if main_path.exists() && index_path.exists() {
            // Attempt to open the main file in the specified mode (append or not)

            let main_file = OpenOptions::new()
                .read(true) // Open in append mode
                .write(true) // Open in append mode
                .open(&main_path)?;

            // Retrieve file size and convert to `u32` for `main_file`, if possible
            let size = main_file
                .metadata()
                .and_then(|meta| u32::try_from(meta.len()).map_err(|err| Error::new(ErrorKind::Other, err)))
                .unwrap_or(0);
            if size < 1 {
                return Err(Error::new(ErrorKind::UnexpectedEof, format!("File empty main:{main_path:?}")));
            }

            // Initialize the index tree (BPlusTree) - by deserializing an existing one
            let index_tree = BPlusTree::<u32, OffsetPointer>::load(&index_path)?;

            Ok(Self {
                main_path,
                index_path,
                main_file,
                index_tree,
            })
        } else {
            Err(Error::new(ErrorKind::NotFound, format!("Files not found main:{main_path:?} index:{index_path:?}")))
        }
    }

    pub fn garbage_collect(&mut self) -> Result<(), Error> {
        let fragmented = IndexedDocument::read_fragmentation(&mut self.main_file)?;
        if !fragmented {
            return Ok(());
        }

        let gc_main_path = file_utils::append_extension(&self.main_path, ".gc");
        let gc_index_path = file_utils::append_extension(&self.index_path,".gc");
        {
            let mut gc_file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&gc_main_path)?;

            let mut key_offset = Vec::<(u32, OffsetPointer)>::new();
            self.index_tree.traverse(|keys, values| {
                keys.iter().zip(values.iter()).for_each(|(&key, &offset)| key_offset.push((key, offset)));
            });

            let fragmented_byte = 0u8.to_le_bytes();
            gc_file.write_all(&fragmented_byte)?;

            let mut gc_offset = 1usize; // offset is 1 because of fragment bit
            let mut buffer: Vec<u8> = Vec::with_capacity(4096);
            let mut size_bytes = [0u8; 4];
            for (key, offset) in key_offset {
                // read old content
                self.main_file.seek(SeekFrom::Start(u64::from(offset)))?;
                self.main_file.read_exact(&mut size_bytes)?;
                let buf_size = u32::from_le_bytes(size_bytes) as usize;
                // ensure buffer capacity
                if buffer.capacity() < buf_size {
                    buffer.reserve(buf_size - buffer.capacity());
                }
                buffer.resize(buf_size, 0u8);
                self.main_file.read_exact(&mut buffer[0..buf_size])?;

                gc_file.write_all(&size_bytes)?;
                gc_file.write_all(&buffer[0..buf_size])?;

                let pointer = u32::try_from(gc_offset).map_err(|err| Error::new(ErrorKind::Other, err))?;
                self.index_tree.insert(key, pointer);
                gc_offset += size_bytes.len() + buf_size; // gc_file.stream_position();
            }

            gc_file.flush()?;
            self.index_tree.store(&gc_index_path)?;
        }

        let _ = std::fs::remove_file(&self.main_path);
        let _ = std::fs::remove_file(&self.index_path);

        std::fs::rename(&gc_main_path, &self.main_path)?;
        std::fs::rename(&gc_index_path, &self.index_path)?;

        Ok(())
    }
}

