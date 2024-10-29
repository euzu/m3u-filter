use std::array::TryFromSliceError;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::path::Path;

use flate2::Compression;
use log::error;
use serde::{Deserialize, Serialize};

const BINCODE_OVERHEAD: usize = 4;
const BLOCK_SIZE: usize = 4096;
const POINTER_SIZE: usize = size_of::<Option<u64>>();

fn is_multiple_of_block_size(file: &File) -> io::Result<bool> {
    let file_size = file.metadata()?.len(); // Get the file size in bytes
    Ok(file_size % (BLOCK_SIZE as u64) == 0) // Check if file size is a multiple of BLOCK_SIZE
}

fn is_file_valid(file: File) -> io::Result<File> {
    match is_multiple_of_block_size(&file) {
        Ok(valid) => {
            if !valid {
                return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Tree file has to be multiple of block size {BLOCK_SIZE}")));
            }
        }
        Err(err) => return Err(err)
    }
    Ok(file)
}

#[inline]
fn u32_from_bytes(bytes: &[u8]) -> io::Result<u32> {
    Ok(u32::from_le_bytes(bytes.try_into().map_err(|e: TryFromSliceError| io::Error::new(io::ErrorKind::Other, e.to_string()))?))
}

#[inline]
fn bincode_serialize<T: ?Sized>(value: &T) -> io::Result<Vec<u8>>
where
    T: serde::Serialize,
{
    bincode::serialize(value).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
}

#[inline]
fn bincode_deserialize<T: ?Sized>(value: &[u8]) -> io::Result<T>
where
    T: for<'a> serde::Deserialize<'a>,
{
    bincode::deserialize(value).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
}


fn get_entry_index_upper_bound<K, V>(keys: &Vec<K>, key: &K) -> usize
where
    K: Ord + Serialize + for<'de> Deserialize<'de> + Clone,
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    let mut left = 0;
    let mut right = keys.len();
    while left < right {
        let mid = left + ((right - left) >> 1);
        if &keys[mid] <= key {
            left = mid + 1;
        } else {
            right = mid;
        }
    }
    left
}


#[derive(Serialize, Deserialize, Debug, Clone)]
struct BPlusTreeNode<K, V> {
    keys: Vec<K>,
    children: Vec<BPlusTreeNode<K, V>>,
    is_leaf: bool,
    values: Vec<V>, // only used in leaf nodes
}

impl<K, V> BPlusTreeNode<K, V>
where
    K: Ord + Serialize + for<'de> Deserialize<'de> + Clone,
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    #[inline]
    fn new(is_leaf: bool) -> Self {
        BPlusTreeNode {
            is_leaf,
            keys: vec![],
            children: vec![],
            values: vec![],
        }
    }

    #[inline]
    fn is_overflow(&self, order: usize) -> bool {
        self.keys.len() > order
    }

    #[inline]
    fn get_median_index(order: usize) -> usize {
        order >> 1
    }

    fn find_leaf_entry<'a>(&self, node: &'a BPlusTreeNode<K, V>) -> &'a K {
        if node.is_leaf {
            node.keys.get(0).unwrap()
        } else {
            let child = node.children.get(0).unwrap();
            self.find_leaf_entry(child)
        }
    }

    #[allow(dead_code)]
    fn query(&self, key: &K) -> Option<&V> {
        if self.is_leaf {
            return match self.keys.binary_search(&key) {
                Ok(idx) => self.values.get(idx),
                Err(_) => None,
            };
        }
        let node = self.children.get(self.get_entry_index_upper_bound(key)).unwrap();
        node.query(key)
    }

    fn get_equal_entry_index(&self, key: &K) -> Option<usize>
    where
        K: Ord,
    {
        let mut left = 0;
        let mut right = self.keys.len().checked_sub(1)?;
        while left <= right {
            let mid = left + ((right - left) >> 1);
            let mid_key = &self.keys[mid];
            if mid_key == key {
                return Some(mid);
            } else if mid_key > key {
                right = mid.checked_sub(1)?;
            } else {
                left = mid + 1;
            }
        }
        None
    }

    fn get_entry_index_upper_bound(&self, key: &K) -> usize {
        get_entry_index_upper_bound::<K, V>(&self.keys, key)
    }

    fn insert(&mut self, key: K, v: V, inner_order: usize, leaf_order: usize) -> Option<BPlusTreeNode<K, V>> {
        if self.is_leaf {
            if let Ok(pos) = self.keys.binary_search(&key) {
                self.values[pos] = v;
                return None;
            }
            if let Some(eq_entry_index) = self.get_equal_entry_index(&key) {
                self.values.insert(eq_entry_index, v);
                return None;
            }
            let pos = self.get_entry_index_upper_bound(&key);
            self.keys.insert(pos, key);
            self.values.insert(pos, v);
            if self.is_overflow(leaf_order) {
                return Some(self.split(leaf_order));
            }
        } else {
            let pos = self.get_entry_index_upper_bound(&key);
            let child = self.children.get_mut(pos).unwrap();
            let node = child.insert(key.clone(), v, inner_order, leaf_order);
            if node.is_some() {
                let leaf_key = self.find_leaf_entry(node.as_ref().unwrap());
                let idx = self.get_entry_index_upper_bound(leaf_key);
                if let Err(_) = self.keys.binary_search(&key) {
                    self.keys.insert(idx, leaf_key.clone());
                    self.children.insert(idx + 1, node.unwrap());
                    if self.is_overflow(inner_order) {
                        return Some(self.split(inner_order));
                    }
                }
            }
        }
        None
    }

    fn split(&mut self, order: usize) -> BPlusTreeNode<K, V> {
        let median = BPlusTreeNode::<K, V>::get_median_index(order);
        if self.is_leaf {
            let mut node = BPlusTreeNode::new(true);
            node.keys = self.keys.split_off(median);
            node.values = self.values.split_off(median);
            node
        } else {
            let mut node = BPlusTreeNode::new(false);
            node.keys = self.keys.split_off(median + 1);
            node.children = self.children.split_off(median + 1);
            self.children.push(node.children.get(0).unwrap().clone());
            node
        }
    }

    pub(crate) fn traverse<F>(&self, visit: &mut F)
    where
        F: FnMut(&Vec<K>, &Vec<V>),
    {
        if self.is_leaf {
            visit(&self.keys, &self.values);
        }
        self.children.iter().for_each(|child| child.traverse(visit));
    }

    fn serialize_to_block<W: Write + Seek>(&self, file: &mut W, buffer: &mut Vec<u8>, offset: u64) -> io::Result<u64> {
        let mut current_offset = offset;
        let buffer_slice = &mut buffer[..];

        // Write node type (leaf or internal)
        buffer_slice[0] = if self.is_leaf { 1u8 } else { 0u8 };
        let mut write_pos = 1;

        // Serialize and write keys
        let keys_encoded = bincode_serialize(&self.keys)?;
        let keys_bytes_len = keys_encoded.len();
        buffer_slice[write_pos..write_pos + 4].copy_from_slice(&(keys_bytes_len as u32).to_le_bytes());
        write_pos += 4;
        buffer_slice[write_pos..write_pos + keys_bytes_len].copy_from_slice(&keys_encoded);
        write_pos += keys_bytes_len;

        // If leaf, serialize and write values
        if self.is_leaf {
            let values_encoded = bincode_serialize(&self.values)?;
            let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), Compression::fast());
            encoder.write_all(&values_encoded)?;
            let compressed_bytes = encoder.finish()?;
            let values_bytes_len = compressed_bytes.len();
            buffer_slice[write_pos..write_pos + 4].copy_from_slice(&(values_bytes_len as u32).to_le_bytes());
            write_pos += 4;
            buffer_slice[write_pos..write_pos + values_bytes_len].copy_from_slice(&compressed_bytes);
            write_pos += values_bytes_len;
        }

        // Write the complete buffer to file, do not optimize to real filled size,
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(&buffer_slice[..BLOCK_SIZE])?; // use BLOCK_SIZE
        current_offset += BLOCK_SIZE as u64;

        if !self.is_leaf {
            let pointer_offset = offset + write_pos as u64;
            let mut pointer = Vec::with_capacity(self.children.len());
            for child in &self.children {
                pointer.push(current_offset);
                current_offset = child.serialize_to_block(file, buffer, current_offset)?;
            }

            let pointer_encoded = bincode_serialize(&pointer)?;
            let pointer_bytes_len = pointer_encoded.len() as u32;

            file.seek(SeekFrom::Start(pointer_offset))?;
            file.write_all(&pointer_bytes_len.to_le_bytes())?;
            file.write_all(&pointer_encoded)?;
        }

        Ok(current_offset)
    }

    fn deserialize_from_block<R: Read + Seek>(file: &mut R, buffer: &mut Vec<u8>, offset: u64, nested: bool) -> io::Result<(Self, Option<Vec<u64>>)> {
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buffer)?;

        // Read the node type directly from buffer
        let is_leaf = buffer[0] == 1u8;
        let mut read_pos = 1;

        // Deserialize keys
        let keys_length = u32_from_bytes(&buffer[read_pos..read_pos + 4])? as usize;
        read_pos += 4;
        let keys: Vec<K> = bincode_deserialize(&buffer[read_pos..read_pos + keys_length])?;
        read_pos += keys_length;

        // Deserialize values if leaf node
        let values = if is_leaf {
            let values_length = u32_from_bytes(&buffer[read_pos..read_pos + 4])? as usize;
            read_pos += 4;
            let compressed_bytes = &buffer[read_pos..read_pos + values_length];
            let mut decoder = flate2::write::ZlibDecoder::new(Vec::new());
            decoder.write_all(&compressed_bytes)?;
            let values_bytes = decoder.finish()?;
            let values: Vec<V> = bincode_deserialize(&values_bytes)?;
            read_pos += values_length;
            values
        } else {
            vec![]
        };

        // Deserialize children indices if internal node
        let (children, children_pointer) = if !is_leaf {
            let pointers_length = u32_from_bytes(&buffer[read_pos..read_pos + 4])? as usize;
            read_pos += 4;
            let pointers: Vec<u64> = bincode_deserialize(&buffer[read_pos..read_pos + pointers_length])?;
            if nested {
                let nodes: Result<Vec<BPlusTreeNode<K, V>>, io::Error> = pointers
                    .iter()
                    .map(|pointer| {
                        BPlusTreeNode::<K, V>::deserialize_from_block(file, buffer, *pointer, nested)
                            .map(|(node, _)| node)
                            .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))
                    })
                    .collect();

                (nodes?, None)
            } else {
                (vec![], Some(pointers))
            }
        } else {
            (vec![], None)
        };

        Ok((BPlusTreeNode {
            is_leaf,
            keys,
            values,
            children,
        }, children_pointer))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct BPlusTree<K, V> {
    root: BPlusTreeNode<K, V>,
    inner_order: usize,
    leaf_order: usize,
}

impl<K, V> BPlusTree<K, V>
where
    K: Ord + Serialize + for<'de> Deserialize<'de> + Clone,
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    pub(crate) fn new() -> Self {
        let key_size = size_of::<K>() + POINTER_SIZE + size_of::<bool>() + BINCODE_OVERHEAD;
        let inner_order = BLOCK_SIZE / key_size;
        let leaf_order = BLOCK_SIZE / (key_size + size_of::<V>() + BINCODE_OVERHEAD);
        BPlusTree {
            root: BPlusTreeNode::<K, V>::new(true),
            inner_order,
            leaf_order,
        }
    }

    fn new_with_root(root: BPlusTreeNode::<K, V>) -> Self {
        let key_size = size_of::<K>() + POINTER_SIZE + size_of::<bool>() + BINCODE_OVERHEAD;
        let inner_order = BLOCK_SIZE / key_size;
        let leaf_order = BLOCK_SIZE / (key_size + size_of::<V>() + BINCODE_OVERHEAD);
        BPlusTree {
            root,
            inner_order,
            leaf_order,
        }
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        if self.root.keys.is_empty() {
            self.root.keys.push(key);
            self.root.values.push(value);
            return;
        }

        if let Some(node) = self.root.insert(key, value, self.inner_order, self.leaf_order) {
            let child_key = if node.is_leaf {
                node.keys.get(0).as_ref().unwrap()
            } else {
                node.find_leaf_entry(&node)
            };

            let mut new_root = BPlusTreeNode::<K, V>::new(false);
            new_root.keys.push(child_key.clone());
            new_root.children.push(std::mem::replace(&mut self.root, BPlusTreeNode::new(true))); // `true` als Beispiel für ein Blatt
            new_root.children.push(node);

            self.root = new_root;
        }
    }

    #[allow(dead_code)]
    pub(crate) fn query(&self, key: &K) -> Option<&V> {
        self.root.query(key)
    }

    pub(crate) fn serialize(&self, filepath: &Path) -> io::Result<u64> {
        let mut file = OpenOptions::new().write(true).create(true).open(filepath)?;
        let mut buffer = vec![0u8; BLOCK_SIZE];
        let result = self.root.serialize_to_block(&mut file, &mut buffer, 0u64);
        file.flush()?;
        result
    }

    pub(crate) fn deserialize(filepath: &Path) -> io::Result<Self> {
        let mut file = File::open(filepath)?;
        if is_multiple_of_block_size(&file).is_err() {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("Tree file has to be multiple of block size {BLOCK_SIZE}")));
        }
        let mut buffer = vec![0u8; BLOCK_SIZE];
        let (root, _) = BPlusTreeNode::deserialize_from_block(&mut file, &mut buffer, 0, true)?;
        Ok(BPlusTree::new_with_root(root))
    }

    pub(crate) fn traverse<F>(&self, mut visit: F)
    where
        F: FnMut(&Vec<K>, &Vec<V>),
    {
        self.root.traverse(&mut visit);
    }
}

fn query_tree<K, V>(file: &mut File, key: &K) -> Option<V>
where
    K: Ord + Serialize + for<'de> Deserialize<'de> + Clone,
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    let mut offset = 0;
    let mut buffer = vec![0u8; BLOCK_SIZE];
    loop {
        match BPlusTreeNode::<K, V>::deserialize_from_block(file, &mut buffer, offset, false) {
            Ok((node, pointers)) => {
                if node.is_leaf {
                    return match node.keys.binary_search(key) {
                        Ok(idx) => node.values.get(idx).cloned(),
                        Err(_) => None,
                    };
                }
                let child_idx = get_entry_index_upper_bound::<K, V>(&node.keys, key);
                offset = *pointers.unwrap().get(child_idx).unwrap();
            }
            Err(err) => {
                error!("Failed to read id tree from file {err}");
                return None;
            }
        };
    }
}

pub(crate) struct BPlusTreeQuery<K, V> {
    file: File,
    _marker_k: PhantomData<K>,
    _marker_v: PhantomData<V>,
}

impl<K, V> BPlusTreeQuery<K, V>
where
    K: Ord + Serialize + for<'de> Deserialize<'de> + Clone,
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    pub(crate) fn try_new(filepath: &Path) -> io::Result<Self> {
        let file = is_file_valid(File::open(filepath)?)?;
        Ok(BPlusTreeQuery {
            file,
            _marker_k: Default::default(),
            _marker_v: Default::default(),
        })
    }

    pub(crate) fn query(&mut self, key: &K) -> Option<V> {
        query_tree(&mut self.file, key)
    }
}

pub(crate) struct BPlusTreeUpdate<K, V> {
    file: File,
    _marker_k: PhantomData<K>,
    _marker_v: PhantomData<V>,
}

impl<K, V> BPlusTreeUpdate<K, V>
where
    K: Ord + Serialize + for<'de> Deserialize<'de> + Clone,
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    pub(crate) fn try_new(filepath: &Path) -> io::Result<Self> {
        if !filepath.exists() {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("File not found {}", filepath.to_str().unwrap_or("?"))));
        }
        let file = OpenOptions::new()
            .write(true)
            .read(true)
            .open(filepath)?;
        let file = is_file_valid(file)?;
        Ok(BPlusTreeUpdate {
            file,
            _marker_k: Default::default(),
            _marker_v: Default::default(),
        })
    }

    pub(crate) fn query(&mut self, key: &K) -> Option<V> {
        query_tree(&mut self.file, key)
    }

    fn serialize_node(&mut self, offset: u64, node: BPlusTreeNode<K, V>) -> io::Result<u64> {
        let mut buffer = vec![0u8; BLOCK_SIZE];
        let result = node.serialize_to_block(&mut self.file, &mut buffer, offset);
        self.file.flush()?;
        result
    }

    pub(crate) fn update(&mut self, key: &K, value: V) -> io::Result<u64> {
        let mut offset = 0;
        let mut buffer = vec![0u8; BLOCK_SIZE];
        loop {
            match BPlusTreeNode::<K, V>::deserialize_from_block(&mut self.file, &mut buffer, offset, false) {
                Ok((mut node, pointers)) => {
                    if node.is_leaf {
                        return match node.keys.binary_search(key) {
                            Ok(idx) => {
                                let old_value = node.values.get(idx);
                                if let Some(_) = old_value {
                                    node.values[idx] = value;
                                    return self.serialize_node(offset, node);
                                }
                                Err(io::Error::new(io::ErrorKind::NotFound, "Entry not found"))
                            }
                            Err(_) => Err(io::Error::new(io::ErrorKind::NotFound, "Entry not found")),
                        };
                    }
                    let child_idx = get_entry_index_upper_bound::<K, V>(&node.keys, key);
                    offset = *pointers.unwrap().get(child_idx).unwrap();
                }
                Err(err) => {
                    error!("Failed to read id tree from file {err}");
                    return Err(io::Error::new(io::ErrorKind::NotFound, format!("Failed to read id tree from file {err}")));
                }
            };
        }
    }
}


#[cfg(test)]
mod tests {
    use std::io;
    use std::path::PathBuf;

    use serde::{Deserialize, Serialize};

    use crate::repository::bplustree::{BPlusTree, BPlusTreeQuery, BPlusTreeUpdate};

    // Example usage with a simple struct
    #[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
    struct Record {
        id: u32,
        data: String,
    }

    #[test]
    fn insert_test() -> io::Result<()> {
        let mut tree = BPlusTree::<u32, Record>::new();
        for i in 0u32..=500 {
            tree.insert(i, Record {
                id: i,
                data: format!("Entry {i}"),
            });
        }

        // // Traverse the tree
        // tree.traverse(|node| {
        //     println!("Node: {:?}", node);
        // });

        let filepath = PathBuf::from("/tmp/tree.bin");
        // Serialize the tree to a file
        tree.serialize(&filepath)?;

        // Deserialize the tree from the file
        tree = BPlusTree::<u32, Record>::deserialize(&filepath)?;

        // Query the tree
        for i in 0u32..=500 {
            let found = tree.query(&i);
            assert!(found.is_some(), "Entry {} not found", i);
            assert!(found.unwrap().eq(&Record {
                id: i,
                data: format!("Entry {i}"),
            }), "Entry {} not found", i);
        }

        let mut tree_query: BPlusTreeQuery<u32, Record> = BPlusTreeQuery::try_new(&filepath)?;
        for i in 0u32..=500 {
            let found = tree_query.query(&i);
            assert!(found.is_some(), "Entry {} not found", i);
            let entry = found.unwrap();
            assert!(entry.eq(&Record {
                id: i,
                data: format!("Entry {i}"),
            }), "Entry {} not found", i);
        }

        let mut tree_update: BPlusTreeUpdate<u32, Record> = BPlusTreeUpdate::try_new(&filepath)?;
        for i in 0u32..=500 {
            if let Some(record) = tree_update.query(&i) {
                let new_record = Record {
                    id: record.id,
                    data: format!("Entry {}", record.id + 9000),
                };
                tree_update.update(&i, new_record)?;
            } else {
                assert!(false, "Entry {} not found", i);
            }
        }

        let mut tree_query: BPlusTreeQuery<u32, Record> = BPlusTreeQuery::try_new(&filepath)?;
        for i in 0u32..=500 {
            let found = tree_query.query(&i);
            assert!(found.is_some(), "Entry {} not found", i);
            let entry = found.unwrap();
            let expected = Record {
                id: i,
                data: format!("Entry {}", i + 9000),
            };
            assert!(entry.eq(&expected), "Entry not equal {:?} != {:?}", entry, expected);
        }

        Ok(())
    }

    #[test]
    fn insert_dulplicate_test() -> io::Result<()> {
        let mut tree = BPlusTree::<u32, Record>::new();
        for i in 0u32..=500 {
            tree.insert(i, Record {
                id: i,
                data: format!("Entry {i}"),
            });
        }
        for i in 0u32..=500 {
            tree.insert(i, Record {
                id: i,
                data: format!("Entry {}", i+1),
            });
        }

        tree.traverse(|keys, values| {
            keys.iter().zip(values.iter()).for_each(|(k, v)| {
                assert!(format!("Entry {}", k+1).eq(&v.data), "Wrong entry")
            });
        });


        Ok(())
    }
}