use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};

use serde::{Deserialize, Serialize};

const BINCODE_OVERHEAD: usize = 4;
const BLOCK_SIZE: usize = 4096;
const POINTER_SIZE: usize = size_of::<Option<u64>>();
//
// #[derive(Serialize, Deserialize, Debug, Clone)]
// struct LeafKeyValue<K, V> {
//     key: K,
//     value: V,
// }
//
// #[derive(Serialize, Deserialize, Debug, Clone)]
// struct InnerKeyValue<K, V> {
//     key: K,
//     value: V,
//     children: u64
// }
//

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
    fn new(is_leaf: bool) -> Self {
        BPlusTreeNode {
            is_leaf,
            keys: vec![],
            children: vec![],
            values: vec![],
        }
    }

    fn is_overflow(&self, order: usize) -> bool {
        self.keys.len() > order
    }

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

    fn query(&self, k: &K) -> Option<&V> {
        if self.is_leaf {
            return match self.keys.binary_search(&k) {
                Ok(idx) => self.values.get(idx),
                Err(_) => None,
            };
        }
        let node = self.children.get(self.get_entry_index_upper_bound(k)).unwrap();
        node.query(k)
    }

    fn get_equal_entry_index(&self, key: &K) -> Option<usize> {
        let mut l = 0;
        let mut r = self.keys.len() - 1;
        while l <= r {
            let mid = l + ((r - l) >> 1);
            let mid_key = self.keys.get(mid).unwrap();
            if mid_key == key {
                return Some(mid);
            } else if mid_key > key {
                r = mid - 1;
            } else {
                l = mid + 1;
            }
        }
        return None;
    }

    fn get_entry_index_upper_bound(&self, key: &K) -> usize {
        let mut l = 0;
        let mut r = self.keys.len();
        while l < r {
            let mid = l + ((r - l) >> 1);
            let mid_key = self.keys.get(mid).unwrap();
            if mid_key <= key {
                l = mid + 1;
            } else {
                r = mid;
            }
        }
        return l;
    }

    fn insert(&mut self, k: K, v: V, inner_order: usize, leaf_order: usize) -> Option<BPlusTreeNode<K, V>> {
        if self.is_leaf {
            if let Some(eq_entry_index) = self.get_equal_entry_index(&k) {
                self.values.insert(eq_entry_index, v);
                return None;
            }
            let pos = self.get_entry_index_upper_bound(&k);
            self.keys.insert(pos, k);
            self.values.insert(pos, v);
            if self.is_overflow(leaf_order) {
                return Some(self.split(leaf_order));
            }
        } else {
            let pos = self.get_entry_index_upper_bound(&k);
            let child = self.children.get_mut(pos).unwrap();
            let node = child.insert(k, v, inner_order, leaf_order);
            if node.is_some() {
                let key = self.find_leaf_entry(node.as_ref().unwrap());
                let idx = self.get_entry_index_upper_bound(key);
                self.keys.insert(idx, key.clone());
                self.children.insert(idx + 1, node.unwrap());
                if self.is_overflow(inner_order) {
                    return Some(self.split(inner_order));
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

    // pub(crate) fn traverse<F>(&self, visit: &mut F)
    // where
    //     F: FnMut(&BPlusTreeNode<K, V>),
    // {
    //     visit(self);
    //     self.children.iter().for_each(|child| child.traverse(visit));
    // }

    fn serialize_to_blocks<W: Write + Seek>(&self, file: &mut W, buffer: &mut Vec<u8>, offset: u64) -> io::Result<u64> {
        let mut current_offset = offset;
        let mut cursor = io::Cursor::new(&mut *buffer);

        cursor.write_all(if self.is_leaf { &[1u8] } else { &[0u8] }).expect("Failed to serialize node type");

        let keys_encoded = bincode::serialize(&self.keys).expect("Failed to serialize keys");
        let keys_bytes = keys_encoded.len() as u32;
        cursor.write_all(&keys_bytes.to_le_bytes()).expect("Failed to write keys length");
        cursor.write_all(&keys_encoded).expect("Failed to write keys");

        if self.is_leaf {
            let values_encoded = bincode::serialize(&self.values).expect("Failed to serialize values");
            cursor.write_all(&(values_encoded.len() as u32).to_le_bytes()).expect("Failed to write values length");
            cursor.write_all(&values_encoded).expect("Failed to write values");
        }

        let cursor_pos = cursor.position();
        cursor.flush().expect("failed to flush cursor");
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(&buffer)?;
        current_offset += BLOCK_SIZE as u64;

        if !self.is_leaf {
            let pointer_offset = offset + cursor_pos as u64;
            let mut pointer = vec![];
            for  child in &self.children {
                pointer.push(current_offset);
                current_offset = child.serialize_to_blocks(file, buffer, current_offset)?;
            }

            let pointer_encoded = bincode::serialize(&pointer).expect("Failed to encode pointer");
            let pointer_bytes = pointer_encoded.len() as u32;

            file.seek(SeekFrom::Start(pointer_offset))?;
            file.write_all(&pointer_bytes.to_le_bytes())?;
            file.write_all(&pointer_encoded)?;
        }

        Ok(current_offset)
    }

    fn deserialize_from_blocks<R: Read + Seek>(file: &mut R, buffer: &mut Vec<u8>, offset: u64) -> io::Result<Self> {
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buffer)?;

        let mut cursor = io::Cursor::new(&mut *buffer);
        // Read the node type
        let mut node_type = [0u8; 1];
        cursor.read_exact(&mut node_type)?;
        let is_leaf = u8::from_le_bytes(node_type) == 1u8;

        // Deserialize keys
        let mut length_bytes = [0u8; 4];
        cursor.read_exact(&mut length_bytes)?;
        let keys_length = u32::from_le_bytes(length_bytes) as usize;

        let mut keys_buffer: Vec<u8> = vec![0; keys_length];
        cursor.read_exact(&mut keys_buffer)?;
        let keys: Vec<K> = bincode::deserialize_from(&*keys_buffer).expect("Failed to deserialize keys");

        // Deserialize values if leaf node
        let values = if is_leaf {
            cursor.read_exact(&mut length_bytes)?;
            let values_length = u32::from_le_bytes(length_bytes) as usize;
            let mut values_buffer: Vec<u8> = vec![0; values_length];
            cursor.read_exact(&mut values_buffer)?;
            let values: Vec<V> = bincode::deserialize_from(&*values_buffer).expect("Failed to deserialize values");
            values
        } else {
            vec![]
        };

        // Deserialize children indices if internal node
        let children = if !is_leaf {
            cursor.read_exact(&mut length_bytes)?;
            let pointers_length = u32::from_le_bytes(length_bytes) as usize;
            let mut pointers_buffer: Vec<u8> = vec![0; pointers_length];
            cursor.read_exact(&mut pointers_buffer)?;
            let pointers: Vec<u64> = bincode::deserialize_from(&*pointers_buffer).expect("Failed to deserialize pointers");
            pointers.iter().map(|pointer| {
                BPlusTreeNode::<K, V>::deserialize_from_blocks(file, buffer, *pointer).expect("failed to deserialize at offset {pointer}")
            }).collect()
        } else {
            vec![]
        };

        Ok(BPlusTreeNode {
            is_leaf,
            keys,
            values,
            children,
        })
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
        if self.root.keys.len() == 0 {
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

    pub(crate) fn query(&self, key: &K) -> Option<&V> {
        self.root.query(key)
    }

    pub(crate) fn serialize(&self, filename: &str) -> io::Result<u64> {
        let mut file = OpenOptions::new().write(true).create(true).open(filename)?;
        let mut buffer = vec![0u8; BLOCK_SIZE];
        self.root.serialize_to_blocks(&mut file, &mut buffer, 0u64)
    }

    pub(crate) fn deserialize(filename: &str) -> io::Result<Self> {
        let mut file = File::open(filename).expect("Failed to open file");
        let mut buffer = vec![0u8; BLOCK_SIZE];
        let root = BPlusTreeNode::deserialize_from_blocks(&mut file, &mut buffer, 0)?;
        Ok(BPlusTree::new_with_root(root))
    }

    // pub(crate) fn traverse<F>(&self, mut visit: F)
    // where
    //     F: FnMut(&BPlusTreeNode<K, V>),
    // {
    //     self.root.traverse(&mut visit);
    // }
}

#[cfg(test)]
mod tests {
    use std::io;

    use serde::{Deserialize, Serialize};

    use crate::utils::bplustree::BPlusTree;

    // Example usage with a simple struct
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Value {
        id: u32,
        data: String,
    }

    #[test]
    fn insert_test() -> io::Result<()> {
        let mut tree = BPlusTree::<u32, String>::new();
        for i in 0u32..=500 {
            tree.insert(i, format!("Entry {i}"));
        }


        // // Traverse the tree
        // tree.traverse(|node| {
        //     println!("Node: {:?}", node);
        // });

        // Serialize the tree to a file
        tree.serialize("/tmp/tree.bin")?;

        // Deserialize the tree from the file
        tree = BPlusTree::<u32, String>::deserialize("/tmp/tree.bin")?;

        // Query the tree
        for i in 0u32..=500 {
            assert!(tree.query(&i).is_some(), "Entry {} not found", i);
        }

        Ok(())
    }
}
