use std::collections::BTreeSet;
use std::path::{Path};
use log::{error, info};
use crate::messaging::{MsgKind, send_message};
use crate::model::config::Config;
use crate::model::playlist::PlaylistGroup;
use crate::utils::file_utils;
use crate::utils::file_utils::sanitize_filename;

pub(crate) fn process_group_watch(cfg: &Config, target_name: &str, pl: &PlaylistGroup) {
    let mut new_tree = BTreeSet::new();
    pl.channels.iter().for_each(|chan| {
        let header = chan.header.borrow();
        let title = if header.title.is_empty() { header.title.to_string() } else { header.name.to_string() };
        new_tree.insert(title);
    });

    let file_name = format!("watch_{target_name}_{}", &pl.title);
    let watch_filename = sanitize_filename(&format!("{file_name}.bin"));
    match file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&watch_filename))) {
        Some(path) => {
            let save_path = path.clone();
            let mut changed = false;
            if path.exists() {
                if let Some(loaded_tree) = load_watch_tree(&path) {
                    // Find elements in set2 but not in set1
                    let added_difference: BTreeSet<String> = new_tree.difference(&loaded_tree).cloned().collect();
                    let removed_difference: BTreeSet<String> = loaded_tree.difference(&new_tree).cloned().collect();
                    if !added_difference.is_empty() || !removed_difference.is_empty() {
                        changed = true;
                        handle_watch_notification(cfg, &added_difference, &removed_difference, target_name, &pl.title);
                    }
                } else {
                    error!("failed to load watch_file {}", &path.to_str().unwrap_or_default());
                    changed = true;
                }
            } else {
                changed = true;
            }
            if changed {
                match save_watch_tree(&save_path, &new_tree) {
                    Ok(()) => {}
                    Err(err) => {
                        error!("failed to write watch_file {}: {}", &save_path.to_str().unwrap_or_default(), err);
                    }
                }
            }
        }
        None => {
            error!("failed to write watch_file {}", &watch_filename);
        }
    }
}

fn handle_watch_notification(cfg: &Config, added: &BTreeSet<String>, removed: &BTreeSet<String>, target_name: &str, group_name: &str) {
    let added_entries = added.iter().map(std::string::ToString::to_string).collect::<Vec<String>>().join("\n\t");
    let removed_entries = removed.iter().map(std::string::ToString::to_string).collect::<Vec<String>>().join("\n\t");

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
        let msg = format!("Changes {}/{}\n{}", target_name, group_name, message.join(""));
        info!("{}", &msg);
        send_message(&MsgKind::Watch, &cfg.messaging, &msg);
    }
}

fn load_watch_tree(path: &Path) -> Option<BTreeSet<String>> {
    match std::fs::read(path) {
        Ok(encoded) => {
            let decoded: BTreeSet<String> = bincode::deserialize(&encoded[..]).unwrap();
            Some(decoded)
        }
        Err(_) => None,
    }
}

fn save_watch_tree(path: &Path, tree: &BTreeSet<String>) -> std::io::Result<()> {
    let encoded: Vec<u8> = bincode::serialize(&tree).unwrap();
    std::fs::write(path, encoded)
}

