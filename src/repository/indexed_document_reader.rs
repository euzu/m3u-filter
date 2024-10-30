use std::convert::TryFrom;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::path::Path;

use crate::repository::bplustree::BPlusTreeQuery;
use crate::repository::indexed_document_writer::OffsetPointer;

fn get_offset(index_path: &Path, doc_id: u32) -> Result<u64, Error> {
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

fn read_content_size(main_file: &mut File) -> Result<usize, Error> {
    let mut size_bytes = [0u8; 4];
    main_file.read_exact(&mut size_bytes)?;
    let buf_size = u32::from_le_bytes(size_bytes) as usize;
    Ok(buf_size)
}

pub(in crate::repository) struct IndexedDocumentReader<T> {
    main_file: File,
    cursor: u32,
    size: u32,
    failed: bool,
    t_buffer: Vec<u8>,
    t_type: PhantomData<T>,
}

impl<T: serde::de::DeserializeOwned> IndexedDocumentReader<T> {
    pub fn new(main_path: &Path) -> Result<IndexedDocumentReader<T>, Error> {
        if main_path.exists() {
            match File::open(main_path) {
                Ok(main_file) => {
                    let size = match main_file.metadata() {
                        Ok(metadata) => {
                            usize::try_from(metadata.len()).map_err(|err| Error::new(ErrorKind::Other, err))?
                        }
                        Err(_e) => 0,
                    };

                    Ok(Self {
                        main_file,
                        cursor: 0,
                        size: u32::try_from(size).map_err(|err| Error::new(ErrorKind::Other, err))?,
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
        !self.failed && self.cursor < self.size
    }
    pub fn read_next(&mut self) -> Result<Option<T>, Error> {
        if !self.has_next() {
            return Ok(None);
        }
        // read content-size
        let buf_size: usize = read_content_size(&mut self.main_file)?;
        self.cursor += 4;
        // resize buffer if necessary
        if self.t_buffer.capacity() < buf_size {
            self.t_buffer.reserve(buf_size - self.t_buffer.capacity());
        }
        self.t_buffer.resize(buf_size, 0u8);
        // read content
        self.main_file.read_exact(&mut self.t_buffer[0..buf_size])?;
        self.cursor += u32::try_from(buf_size).map_err(|err| Error::new(ErrorKind::Other, err))?;
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
            let offset = get_offset(index_path, doc_id)?;
            let mut main_file = File::open(main_path)?;
            main_file.seek(SeekFrom::Start(offset))?;
            let buf_size = read_content_size(&mut main_file)?;
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
