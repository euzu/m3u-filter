use std::fs::File;
use std::io::{self};
use flate2::Compression;
use flate2::write::{GzEncoder};
use flate2::read::{GzDecoder};

use serde::{Deserialize, Serialize};

const T: usize = 3; // Minimum degree (T), meaning each node can contain at most 2*T - 1 keys

#[derive(Serialize, Deserialize, Debug, Clone)]
struct KeyValue<K, V> {
    key: K,
    value: V,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BPlusTreeNode<K, V> {
    keys: Vec<K>,
    children: Vec<Option<BPlusTreeNode<K, V>>>,
    is_leaf: bool,
    values: Vec<Option<V>>, // only used in leaf nodes
}

impl<K, V> BPlusTreeNode<K, V>
where
    K: Ord + Serialize + for<'de> Deserialize<'de> + Clone,
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    fn new(is_leaf: bool) -> Self {
        BPlusTreeNode {
            keys: vec![],
            children: vec![],
            is_leaf,
            values: vec![],
        }
    }

    fn insert_non_full(&mut self, key: K, value: V) {
        let pos = self.keys.binary_search(&key).unwrap_or_else(|pos| pos);

        if self.is_leaf {
            self.keys.insert(pos, key);
            self.values.insert(pos, Some(value));
        } else {
            let mut idx = pos;
            if self.children[pos].as_mut().unwrap().keys.len() == 2 * T - 1 {
                self.split_child(pos);
                if key > self.keys[pos] {
                    idx = pos + 1;
                }
            }
            self.children[idx].as_mut().unwrap().insert_non_full(key, value);
        }
    }

    fn split_child(&mut self, pos: usize) {
        let t = T - 1;
        let mut new_node = BPlusTreeNode::new(self.children[pos].as_ref().unwrap().is_leaf);
        let mut old_node = self.children[pos].take().unwrap();

        self.keys.insert(pos, old_node.keys.remove(t));
        self.children.insert(pos + 1, Some(new_node.clone()));

        if old_node.is_leaf {
            new_node.keys = old_node.keys.split_off(t);
            new_node.values = old_node.values.split_off(t);
        } else {
            new_node.keys = old_node.keys.split_off(t + 1);
            new_node.children = old_node.children.split_off(t + 1);
        }

        self.children[pos] = Some(old_node);
        self.children[pos + 1] = Some(new_node);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct BPlusTree<K, V> {
    root: BPlusTreeNode<K, V>,
}

impl<K, V> BPlusTree<K, V>
where
    K: Ord + Serialize + for<'de> Deserialize<'de> + Clone,
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    pub(crate) fn new() -> Self {
        BPlusTree {
            root: BPlusTreeNode::new(true),
        }
    }

    pub(crate) fn insert(&mut self, key: K, value: V) {
        if self.root.keys.len() == 2 * T - 1 {
            let mut new_root = BPlusTreeNode::new(false);
            new_root.children.push(Some(self.root.clone()));
            new_root.split_child(0);
            self.root = new_root;
        }
        self.root.insert_non_full(key, value);
    }

    pub(crate) fn query(&self, key: &K) -> Option<V> {
        let mut node = &self.root;
        while !node.is_leaf {
            let pos = match node.keys.binary_search(key) {
                Ok(pos) => return node.values[pos].clone(),
                Err(pos) => pos,
            };
            node = node.children[pos].as_ref().unwrap();
        }

        match node.keys.binary_search(key) {
            Ok(pos) => node.values[pos].clone(),
            Err(_) => None,
        }
    }

    pub(crate) fn serialize_to_file(&self, filename: &str) -> io::Result<()> {
        let file = File::create(filename)?;
        let encoder = GzEncoder::new(file, Compression::default());
        match bincode::serialize_into(encoder, &self) {
            Ok(()) => Ok(()),
            Err(e) => {
                println!("Failed to write bplusstree to disk {e}");
                Ok(())
            }
        }
    }

    // If file exists the file is deserialized, otherweise an empty tree is returned
    pub(crate) fn deserialize_from_file(filename: &str) -> Self {
        match File::open(filename) {
            Ok(file) => {
                let decoder = GzDecoder::new(file);
                let tree: BPlusTree<K, V> = bincode::deserialize_from(decoder).unwrap();
                tree
            }
            Err(_) => BPlusTree::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use serde::{Deserialize, Serialize};
    use crate::repository::bplustree::{BPlusTree};


    // Example usage with a simple struct
    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct Value {
        id: u32,
        data: String,
    }

    #[test]
    fn insert_test() -> io::Result<()> {
        let mut tree = BPlusTree::new();
        tree.insert("abc".to_string(), Value { id: 16, data: "one".to_string() });
        tree.insert("def".to_string(), Value { id: 32, data: "two".to_string() });
        tree.insert("ghi".to_string(), Value { id: 64, data: "three".to_string() });

        // Serialize the tree to a file
        tree.serialize_to_file("/tmp/tree.bin")?;

        // Deserialize the tree from the file
        let tree = BPlusTree::<String, Value>::deserialize_from_file("/tmp/tree.bin");

        // Query the tree
        if let Some(value) = tree.query(&("ghi".to_string())) {
            println!("Found: {:?}", value);
        } else {
            println!("Not found");
        }

        Ok(())
    }
}