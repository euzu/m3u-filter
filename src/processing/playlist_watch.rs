use std::collections::BTreeSet;
use std::path::PathBuf;
use log::{error, info};
use regex::Regex;
use crate::messaging::send_message;
use crate::model::config::Config;
use crate::model::model_m3u::PlaylistGroup;
use crate::utils;

pub(crate) fn process_group_watch(cfg: &Config, target_name: &str, pl: &PlaylistGroup) {
    let mut new_tree = BTreeSet::new();
    pl.channels.iter().for_each(|chan| {
        let header = chan.header.borrow();
        let title = if header.title.is_empty() { header.title.to_string() } else { header.name.to_string() };
        new_tree.insert(title);
    });

    let filename_re = Regex::new(r"[^A-Za-z0-9_-]").unwrap();
    let file_name = format!("watch_{}_{}", target_name, &pl.title);
    let watch_filename = format!("{}.bin", filename_re.replace_all(&file_name, "_")).to_string();
    match utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&watch_filename))) {
        Some(path) => {
            let save_path = path.clone();
            if path.exists() {
                match load_tree(&path) {
                    Some(loaded_tree) => {
                        // Find elements in set2 but not in set1
                        let added_difference: BTreeSet<String> = new_tree.difference(&loaded_tree).cloned().collect();
                        let removed_difference: BTreeSet<String> = loaded_tree.difference(&new_tree).cloned().collect();
                        if !added_difference.is_empty() || !removed_difference.is_empty() {
                            handle_watch_notification(cfg, added_difference, removed_difference);
                        }
                    }
                    None => {
                        error!("failed to load watch_file {}", &path.into_os_string().into_string().unwrap());
                    }
                }
            }
            match save_tree(&save_path, new_tree) {
                Ok(_) => {}
                Err(err) => {
                    error!("failed to write watch_file {}: {}", &save_path.into_os_string().into_string().unwrap(), err)
                }
            }
        }
        None => {
            error!("failed to write watch_file {}", &watch_filename);
        }
    }
}

fn handle_watch_notification(cfg: &Config, added: BTreeSet<String>, removed: BTreeSet<String>) {
    let added_entries = added.iter().map(|name| name.to_string()).collect::<Vec<String>>().join("\n\t");
    let removed_entries = removed.iter().map(|name| name.to_string()).collect::<Vec<String>>().join("\n\t");

    let mut message = vec![];
    if !added_entries.is_empty() {
        message.push("added: [\n\t".to_string());
        message.push(added_entries);
        message.push("\n]\n".to_string());
    }
    if !removed_entries.is_empty() {
        message.push("removed: [\n\t".to_string());
        message.push(removed_entries);
        message.push("\n]\n".to_string());
    }

    if !message.is_empty() {
        info!("{}", message.join("").as_str());
        send_message(&cfg.messaging, message.join("").as_str())
    }
}

fn load_tree(path: &PathBuf) -> Option<BTreeSet<String>> {
    match std::fs::read(path) {
        Ok(encoded) => {
            let decoded: BTreeSet<String> = bincode::deserialize(&encoded[..]).unwrap();
            Some(decoded)
        }
        Err(_) => None,
    }
}

fn save_tree(path: &PathBuf, tree: BTreeSet<String>) -> std::io::Result<()> {
    let encoded: Vec<u8> = bincode::serialize(&tree).unwrap();
    std::fs::write(path, encoded)
}

