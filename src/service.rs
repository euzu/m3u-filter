use std::path::PathBuf;
use crate::{m3u, utils};
use crate::m3u::PlaylistGroup;

pub(crate) fn get_playlist(url: &str, persist_file: Option<PathBuf>) -> Option<Vec<PlaylistGroup>> {
    let lines: Option<Vec<String>> = utils::get_input_content(url, persist_file);
    lines.map_or(None, |l| Some(m3u::decode(&l)))
}
