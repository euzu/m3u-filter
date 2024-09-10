use std::convert::TryFrom;
use std::fs::File;
use std::io::{Error, ErrorKind, Read, Seek, SeekFrom};
use std::marker::PhantomData;
use std::path::Path;

use crate::repository::index_record::IndexRecord;

pub(in crate::repository) struct IndexedDocumentReader<T> {
    main_file: File,
    index_file: File,
    cursor: u32,
    size: u32,
    failed: bool,
    t_buffer: Vec<u8>,
    t_type: PhantomData<T>,
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
                                    usize::try_from(metadata.len()).map_err(|err| Error::new(ErrorKind::Other, err))?
                                }
                                Err(_e) => 0,
                            };
                            Ok(Self {
                                main_file,
                                index_file,
                                cursor: 0,
                                size: u32::try_from(size).map_err(|err| Error::new(ErrorKind::Other, err))?,
                                failed: false,
                                t_buffer: Vec::new(),
                                t_type: PhantomData,
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
            self.cursor += IndexRecord::get_record_size();
            match record {
                Ok(index_record) => {
                    let offset = u64::from(index_record.left);
                    let buf_size = index_record.right as usize;
                    if self.t_buffer.len() < buf_size {
                        self.t_buffer.resize(buf_size, 0u8);
                    }
                    self.main_file.seek(SeekFrom::Start(offset))?;
                    self.main_file.read_exact(&mut self.t_buffer[0..buf_size])?;
                    return match bincode::deserialize::<T>(&self.t_buffer[0..buf_size]) {
                        Ok(value) => Ok(Some(value)),
                        Err(err) => {
                            self.failed = true;
                            Err(Error::new(ErrorKind::Other, format!("Failed to deserialize document {err}")))
                        }
                    };
                }
                Err(err) => {
                    self.failed = true;
                    return Err(Error::new(ErrorKind::Other, format!("Failed to deserialize document {err}")));
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

pub(in crate::repository)  fn read_indexed_item<T>(main_path: &Path, index_path: &Path, offset: u32) -> Result<T, Error>
    where T: ?Sized + serde::de::DeserializeOwned
{
    if main_path.exists() && index_path.exists() {
        let mut index_file = File::open(index_path)?;
        let mut main_file = File::open(main_path)?;
        let index_record = IndexRecord::from_file(&mut index_file, offset)?;
        main_file.seek(SeekFrom::Start(u64::from(index_record.left)))?;
        let mut buffer: Vec<u8> = vec![0; index_record.right as usize];
        main_file.read_exact(&mut buffer)?;
        if let Ok(item) = bincode::deserialize::<T>(&buffer[..]) {
            return Ok(item);
        }
    }
    Err(Error::new(ErrorKind::Other, format!("Failed to read item for offset {} - {}", offset, main_path.to_str().unwrap())))
}