use std::fs::File;
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::path::Path;

use crate::repository::bplustree::{BPlusTree};
use crate::repository::indexed_document::{IndexedDocument, OffsetPointer};



pub(in crate::repository) struct IndexedDocumentReader<T> {
    main_file: File,
    offsets: Vec<OffsetPointer>,
    index: usize,
    failed: bool,
    t_buffer: Vec<u8>,
    t_type: PhantomData<T>,
}

impl<T: serde::de::DeserializeOwned> IndexedDocumentReader<T> {
    pub fn new(main_path: &Path, index_path: &Path) -> Result<IndexedDocumentReader<T>, Error> {
        if main_path.exists() && index_path.exists() {
            let mut offsets = Vec::<OffsetPointer>::new();
            {
                let index_tree = BPlusTree::<u32, OffsetPointer>::load(index_path)?;
                index_tree.traverse(|_, values| {
                    offsets.extend(values);
                });
                offsets.sort_unstable();
            }
            match File::open(main_path) {
                Ok(main_file) => {
                    Ok(Self {
                        main_file,
                        offsets,
                        index: 0,
                        failed: false,
                        t_buffer: Vec::new(),
                        t_type: PhantomData,
                    })
                }
                Err(e) => Err(e)
            }
        } else {
            Err(Error::new(ErrorKind::NotFound, format!("File not found {}",
                                                        main_path.to_str().unwrap())))
        }
    }

    pub fn has_error(&self) -> bool {
        self.failed
    }

    pub fn has_next(&self) -> bool {
        !self.failed && self.index < self.offsets.len()
    }
    pub fn read_next(&mut self) -> Result<Option<T>, Error> {
        if !self.has_next() {
            return Ok(None);
        }
        // read content-size
        self.main_file.seek(SeekFrom::Start(u64::from(self.offsets[self.index])))?;
        self.index += 1;
        let buf_size: usize = IndexedDocument::read_content_size(&mut self.main_file)?;
        // resize buffer if necessary
        if self.t_buffer.capacity() < buf_size {
            self.t_buffer.reserve(buf_size - self.t_buffer.capacity());
        }
        self.t_buffer.resize(buf_size, 0u8);
        // read content
        self.main_file.read_exact(&mut self.t_buffer[0..buf_size])?;
        // deserialize buffer
        match bincode::deserialize::<T>(&self.t_buffer[0..buf_size]) {
            Ok(value) => Ok(Some(value)),
            Err(err) => {
                self.failed = true;
                Err(Error::new(ErrorKind::Other, format!("Failed to deserialize document {err}")))
            }
        }
    }

    pub(in crate::repository) fn read_indexed_item(main_path: &Path, index_path: &Path, doc_id: u32) -> Result<T, Error>
    {
        if main_path.exists() && index_path.exists() {
            // get the offset from index
            let offset = IndexedDocument::get_offset(index_path, doc_id)?;
            let mut main_file = File::open(main_path)?;
            main_file.seek(SeekFrom::Start(offset))?;
            let buf_size = IndexedDocument::read_content_size(&mut main_file)?;
            let mut buffer: Vec<u8> = vec![0; buf_size];
            main_file.read_exact(&mut buffer)?;
            if let Ok(item) = bincode::deserialize::<T>(&buffer) {
                return Ok(item);
            }
        }
        Err(Error::new(ErrorKind::Other, format!("Failed to read item for id {} - {}", doc_id, main_path.to_str().unwrap())))
    }
}

impl<T: serde::de::DeserializeOwned> Iterator for IndexedDocumentReader<T> {
    type Item = T;

    // Implement the next() method
    fn next(&mut self) -> Option<Self::Item> {
        if self.has_next() {
            if let Ok(value) = self.read_next() {
                return value;
            }
        }
        None
    }
}
