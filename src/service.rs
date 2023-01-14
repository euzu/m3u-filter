use std::path::PathBuf;
use crate::{m3u, utils};
use crate::m3u::PlaylistGroup;

pub(crate) fn get_playlist(working_dir: &String, url: &str, persist_file: Option<PathBuf>, verbose: bool) -> Option<Vec<PlaylistGroup>> {
    let lines: Option<Vec<String>> = utils::get_input_content(working_dir, url, persist_file, verbose);
    lines.map_or(None, |l| Some(m3u::decode(&l)))
}
