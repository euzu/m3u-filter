use std::fs::{File, OpenOptions};
use std::io::{BufReader, Error, ErrorKind, Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use log::error;

use crate::repository::bplustree::{BPlusTree, BPlusTreeQuery};
use crate::utils::file_utils;

const BLOCK_SIZE: usize = 4096;
const LEN_SIZE: usize = 4;

pub(in crate::repository) type OffsetPointer = u32;

pub(in crate::repository) struct IndexedDocument {}

impl IndexedDocument {
    pub(in crate::repository) fn read_fragmentation<R: Read + Seek>(file: &mut R) -> std::io::Result<bool> {
        file.seek(SeekFrom::Start(0))?;
        let mut bool_bytes = [0u8];
        file.read_exact(&mut bool_bytes)?;
        Ok(u8::from_le_bytes(bool_bytes) == 1)
    }

    pub(in crate::repository) fn write_fragmentation<W: Write + Seek>(file: &mut W, fragmented: bool) -> std::io::Result<()> {
        file.seek(SeekFrom::Start(0))?;
        let fragmented_byte = if fragmented { 1u8.to_le_bytes() } else { 0u8.to_le_bytes() };
        file.write_all(&fragmented_byte)
    }

    pub(in crate::repository) fn read_content_size<R: Read + Seek>(reader: &mut R) -> Result<usize, Error>
    {
        let mut size_bytes = [0u8; LEN_SIZE];
        reader.read_exact(&mut size_bytes)?;
        let buf_size = u32::from_le_bytes(size_bytes) as usize;
        Ok(buf_size)
    }


    pub(in crate::repository) fn get_offset(index_path: &Path, doc_id: u32) -> Result<u64, Error> {
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
}

////////////////////////////////////////////////////////
//
// IndexedDocumentWriter
//
////////////////////////////////////////////////////////


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
        let encoded_bytes = bincode::serialize(doc).map_err(|_| Error::new(ErrorKind::InvalidData, "Failed to serialize document"))?;
        let mut new_record_appended = false; // do i need to change the index and set the new offset
        if let Some(&offset) = self.index_tree.query(&doc_id) {
            self.main_file.seek(SeekFrom::Start(u64::from(offset)))?;
            let size = IndexedDocument::read_content_size(&mut self.main_file)?;
            if size == encoded_bytes.len() {
                // check if it is equal
                let mut record_buffer = Vec::<u8>::with_capacity(size);
                self.main_file.read_exact(&mut record_buffer)?;
                if record_buffer == encoded_bytes {
                    return Ok(());
                }
            }

            if encoded_bytes.len() > size {
                // does not fit we need to append, file is fragmented
                if !self.fragmented {
                    self.fragmented = true;
                    IndexedDocument::write_fragmentation(&mut self.main_file, true)?;
                }
                self.main_file.seek(SeekFrom::End(0))?;
                new_record_appended = true;
            } else {
                self.main_file.seek(SeekFrom::Start(u64::from(offset)))?;
            }
        } else {
            self.main_file.seek(SeekFrom::End(0))?;
            new_record_appended = true;
        }

        self.dirty = true;

        let encoded_bytes_len = u32::try_from(encoded_bytes.len()).map_err(|err| Error::new(ErrorKind::Other, err))?;
        self.main_file.write_all(&encoded_bytes_len.to_le_bytes())?;
        match file_utils::check_write(&self.main_file.write_all(&encoded_bytes)) {
            Ok(()) => {
                if new_record_appended {
                    self.index_tree.insert(doc_id, self.main_offset);
                    let written_bytes = u32::try_from(encoded_bytes.len() + LEN_SIZE).map_err(|err| Error::new(ErrorKind::Other, err))?;
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

////////////////////////////////////////////////////////
//
// IndexedDocumentReader
//
////////////////////////////////////////////////////////

pub(in crate::repository) struct IndexedDocumentReader<T> {
    main_file: BufReader<File>,
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
                Ok(file) => {
                    Ok(Self {
                        main_file: BufReader::new(file),
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

////////////////////////////////////////////////////////
//
// IndexedDocumentGarbageCollector
//
////////////////////////////////////////////////////////

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
        let gc_index_path = file_utils::append_extension(&self.index_path, ".gc");
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
            let mut buffer: Vec<u8> = Vec::with_capacity(BLOCK_SIZE);
            let mut size_bytes = [0u8; LEN_SIZE];
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

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use crate::repository::indexed_document::{IndexedDocumentGarbageCollector, IndexedDocumentReader, IndexedDocumentWriter};

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

            let size_main_file_1 = std::fs::metadata(&main_path)?.len();

            // update same block, file size should not increase
            for i in 0u32..=500 {
                idw.write_doc(i, &Record {
                    id: i,
                    data: format!("E {}", i),
                })?;
            }

            let size_main_file_2 = std::fs::metadata(&main_path)?.len();
            assert_eq!(size_main_file_1, size_main_file_2, "Failed, the filesize should be the same");

            // fragmentation
            for i in 0u32..=500 {
                idw.write_doc(i, &Record {
                    id: i,
                    data: format!("Entry {}", i + 9000),
                })?;
            }

            let size_main_file_3 = std::fs::metadata(&main_path)?.len();
            assert!(size_main_file_1 < size_main_file_3, "Failed, the filesize should be greater");

            idw.store()?;
        }
        {
            let size_main_file_4 = std::fs::metadata(&main_path)?.len();

            let mut gc = IndexedDocumentGarbageCollector::new(main_path.clone(), index_path.clone())?;
            gc.garbage_collect()?;

            let size_main_file_5 = std::fs::metadata(&main_path)?.len();
            assert!(size_main_file_5 < size_main_file_4, "Failed, the filesize should be less");
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