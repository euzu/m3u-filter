// This module is not included in the build

use std::cmp::max;
use std::io::Error;
use std::path::{Path, PathBuf};

use log::error;
use serde::{Deserialize, Serialize};

use crate::repository::bplustree::BPlusTree;

pub(crate) struct IdMapping<V>
where
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    dirty: bool,
    tree: BPlusTree<u32, V>,
    path: PathBuf,
    max_id: u32,
}

impl<V> IdMapping<V>
where
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    pub(crate) fn new(path: &Path) -> Self {
        let tree: BPlusTree<u32, V> = match BPlusTree::<u32, V>::deserialize(&path) {
            Ok(tree) => tree,
            _ => BPlusTree::<u32, V>::new()
        };

        let mut max_id = 0;
        tree.traverse(|keys, _| {
            match keys.iter().max() {
                None => {}
                Some(max_value) => {
                    max_id = max(max_id, *max_value);
                }
            }
        });

        IdMapping {
            dirty: false,
            tree,
            path: path.to_path_buf(),
            max_id,
        }
    }

    pub(crate) fn insert(&mut self, id: u32, value: V) {
        self.dirty = true;
        self.tree.insert(id, value);
    }

    pub(crate) fn persist(&mut self) -> Result<(), Error> {
        if self.dirty {
            self.tree.serialize(&self.path)?;
        }
        self.dirty = false;
        Ok(())
    }

    pub(crate) fn max_id(&self) -> u32 {
        self.max_id
    }

    pub(crate) fn query(&self, id: u32) -> Option<&V> {
        self.tree.query(&id)
    }
}

impl<V> Drop for IdMapping<V>
where
    V: Serialize + for<'de> Deserialize<'de> + Clone,
{
    fn drop(&mut self) {
        match self.persist() {
            Ok(_) => {}
            Err(err) => {
                error!("Failed to persist id mapping {:?} err:{}", &self.path, err.to_string())
            }
        }
    }
}