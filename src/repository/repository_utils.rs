use std::fs::File;
use std::io::{Error, ErrorKind, Write};
use std::io::{Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use crate::utils::file_utils;
use crate::utils::file_utils::create_file_tuple;

/**
We write the structs with bincode::encode to a file.
To access each entry we need a index file where we can find the
Entries with offset and size of the encoded struct.
This is used for the index file where first entry is the index
of the encoded file, and size is the size of the encoded struct.
 */
struct IndexRecord {
    pub index: u32,
    pub size: u16,
}

impl IndexRecord {
    pub fn from_file(file: &mut File, offset: usize) -> Result<Self, Error> {
        file.seek(SeekFrom::Start(offset as u64))?;
        let mut index_bytes = [0u8; 4];
        let mut size_bytes = [0u8; 2];
        file.read_exact(&mut index_bytes)?;
        file.read_exact(&mut size_bytes)?;
        let index = u32::from_le_bytes(index_bytes);
        let size = u16::from_le_bytes(size_bytes);
        Ok(IndexRecord { index, size })
    }

    // pub fn from_bytes(bytes: &[u8], cursor: &mut usize) -> Result<Self, Error> {
    //     if let Ok(index_bytes) = bytes[*cursor..*cursor + 4].try_into() {
    //         *cursor += 4;
    //         if let Ok(size_bytes) = bytes[*cursor..*cursor + 2].try_into() {
    //             *cursor += 2;
    //             let index = u32::from_le_bytes(index_bytes);
    //             let size = u16::from_le_bytes(size_bytes);
    //             return Ok(IndexRecord { index, size });
    //         }
    //     }
    //     Err(Error::new(ErrorKind::Other, "Failed to read index"))
    // }

    // pub fn as_bytes(&self) -> [u8; 6] {
    //     IndexRecord::to_bytes(self.index, self.size)
    // }

    pub fn to_bytes(index: u32, size: u16) -> [u8;6] {
        let index_bytes: [u8; 4] = index.to_le_bytes();
        let size_bytes: [u8; 2] = size.to_le_bytes();
        let mut combined_bytes: [u8; 6] = [0; 6];
        combined_bytes[..4].copy_from_slice(&index_bytes);
        combined_bytes[4..].copy_from_slice(&size_bytes);
        combined_bytes
    }

    fn get_record_size() -> u16 { 6 }
    fn get_index_offset(index: u32) -> usize { index as usize * 6 }
}

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

pub(crate) struct IndexedDocumentReader<T> {
    main_file: File,
    index_file: File,
    cursor: usize,
    size: usize,
    failed: bool,
    _buffer: Vec<u8>,
    _type: PhantomData<T>,
}

impl<T: ?Sized + serde::de::DeserializeOwned> IndexedDocumentReader<T> {
    pub fn new(main_path: &Path, index_path: &Path) -> Result<IndexedDocumentReader<T>, Error> {
        if main_path.exists() && index_path.exists() {
            match File::open(main_path) {
                Ok(main_file) => {
                    match File::open(index_path) {
                        Ok(index_file) => {
                            let size = match index_file.metadata() {
                                Ok(metadata) => {
                                    metadata.len() as usize
                                }
                                Err(_e) => 0,
                            };
                            Ok(IndexedDocumentReader {
                                main_file,
                                index_file,
                                cursor: 0,
                                size,
                                failed: false,
                                _buffer: Vec::new(),
                                _type: Default::default(),
                            })
                        }
                        Err(e) => Err(e)
                    }
                }
                Err(e) => Err(e)
            }
        } else {
            Err(Error::new(ErrorKind::NotFound, format!("File not found {} or {}",
                                                        main_path.to_str().unwrap(),
                                                        index_path.to_str().unwrap())))
        }
    }

    pub fn has_error(&self) -> bool {
        self.failed
    }

    pub fn has_next(&self) -> bool {
        !self.failed && self.cursor < self.size
    }
    pub fn read_next(&mut self) -> Result<Option<T>, Error> {
        if self.has_next() {
            let record = IndexRecord::from_file(&mut self.index_file, self.cursor);
            self.cursor += IndexRecord::get_record_size() as usize;
            match record {
                Ok(index_record) => {
                    let offset = index_record.index as u64;
                    let buf_size = index_record.size as usize;
                    if self._buffer.len() < buf_size {
                        self._buffer.resize(buf_size, 0u8);
                    }
                    self.main_file.seek(SeekFrom::Start(offset))?;
                    self.main_file.read_exact(&mut self._buffer[0..buf_size])?;
                    return match bincode::deserialize::<T>(&self._buffer[0..buf_size]) {
                        Ok(value) => Ok(Some(value)),
                        Err(err) => {
                            self.failed = true;
                            Err(Error::new(ErrorKind::Other, format!("Failed to deserialize document {}", err)))
                        }
                    };
                }
                Err(err) => {
                    self.failed = true;
                    return Err(Error::new(ErrorKind::Other, format!("Failed to deserialize document {}", err)));
                }
            }
        }
        Ok(None)
    }
}

impl<T: ?Sized + serde::de::DeserializeOwned> Iterator for IndexedDocumentReader<T> {
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

pub(crate) fn read_indexed_item<T>(main_path: &Path, index_path: &Path, offset: u32) -> Result<T, Error>
    where T: ?Sized + serde::de::DeserializeOwned
{
    if main_path.exists() && index_path.exists() {
        let offset = IndexRecord::get_index_offset(offset);
        let mut index_file = File::open(index_path)?;
        let mut main_file = File::open(main_path)?;
        let index_record = IndexRecord::from_file(&mut index_file, offset)?;
        main_file.seek(SeekFrom::Start(index_record.index as u64))?;
        let mut buffer: Vec<u8> = vec![0; index_record.size as usize];
        main_file.read_exact(&mut buffer)?;
        if let Ok(item) = bincode::deserialize::<T>(&buffer[..]) {
            return Ok(item);
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to read item for offset {} - {}", offset, main_path.to_str().unwrap())))
}

