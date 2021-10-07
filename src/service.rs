use std::path::PathBuf;
use crate::{m3u, utils};
use crate::m3u::PlaylistGroup;

pub(crate) fn get_playlist(url: &str, persist_file: Option<PathBuf>) -> Vec<PlaylistGroup> {
    let lines: Vec<String> = utils::get_input_content(url, persist_file);
    m3u::decode(&lines)
}
