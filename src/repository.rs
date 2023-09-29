use polodb_core::Database;
use crate::config::{Config, ConfigTarget};
use crate::model_m3u::PlaylistGroup;
use crate::utils;
use serde::{Deserialize, Serialize};

pub(crate) fn save_playlist(target: &ConfigTarget, cfg: &Config, playlist: &mut Vec<PlaylistGroup>) -> Result<(), std::io::Error> {
    //let mut new_playlist = playlist.to_owned();
    // if let Some(path) = utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(format!("{}.db", &target.name)))) {
    //     let db = Database::open_file(path).unwrap();
    //     db.insert_many()
    // }


    Ok(())
}