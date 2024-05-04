use std::convert::TryInto;
use std::io::Error;
use std::fs::File;
use std::io::{Read, SeekFrom, Seek};

/**
We write the structs with bincode::encode to a file.
To access each entry we need a index file where we can find the
Entries with offset and size of the encoded struct.
This is used for the index file where first entry is the index
of the encoded file, and size is the size of the encoded struct.
*/
pub struct IndexRecord {
    pub index: u32,
    pub size: u32,
}

impl IndexRecord {
    pub fn new(index: u32, size: u32) -> IndexRecord {
        IndexRecord { index, size }
    }

    pub fn from_file(file: &mut File, offset: u64) -> Result<IndexRecord, Error> {
        file.seek(SeekFrom::Start(offset))?;
        let mut index_bytes = [0u8; 4];
        let mut size_bytes = [0u8; 4];
        file.read_exact(&mut index_bytes)?;
        file.read_exact(&mut size_bytes)?;
        let index = u32::from_le_bytes(index_bytes);
        let size = u32::from_le_bytes(size_bytes);
        Ok(IndexRecord { index, size })
    }

    pub fn from_bytes(bytes: &[u8], cursor: &mut usize) -> IndexRecord {
        let index_bytes: [u8; 4] = bytes[*cursor..*cursor + 4].try_into().unwrap();
        *cursor += 4;
        let size_bytes: [u8; 4] = bytes[*cursor..*cursor + 4].try_into().unwrap();
        *cursor += 4;
        let index = u32::from_le_bytes(index_bytes);
        let size = u32::from_le_bytes(size_bytes);
        IndexRecord { index, size }
    }

    pub fn to_bytes(&self) -> [u8; 8] {
        let index_bytes: [u8; 4] = self.index.to_le_bytes();
        let size_bytes: [u8; 4] = self.size.to_le_bytes();
        let mut combined_bytes: [u8; 8] = [0; 8];
        combined_bytes[..4].copy_from_slice(&index_bytes);
        combined_bytes[4..].copy_from_slice(&size_bytes);
        combined_bytes
    }

    pub fn get_index_offset(index: u32) -> u32 { index * 8 }
}