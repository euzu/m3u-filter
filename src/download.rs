use std::path::PathBuf;
use crate::{m3u_parser, utils, xtream_parser};
use crate::config::{Config, ConfigInput};
use crate::model_m3u::{PlaylistGroup, XtreamCluster};

fn prepare_file_path(input: &ConfigInput, working_dir: &String, action: &str, verbose: bool) -> Option<PathBuf> {
    let persist_file: Option<PathBuf> =
        if input.persist.is_empty() { None } else { utils::prepare_persist_path(input.persist.as_str(), action) };
    let file_path =  utils::get_file_path(working_dir, persist_file);
    if verbose {
        println!("persist to file:  {:?}", match &file_path {
            Some(fp) => fp.display().to_string(),
            _ => "".to_string()
        });
    }
    file_path
}


pub(crate) fn get_m3u_playlist(cfg: &Config, input: &ConfigInput, working_dir: &String, verbose: bool) -> Option<Vec<PlaylistGroup>> {
    let url = input.url.as_str();
    let file_path = prepare_file_path(input, working_dir, "", verbose);
    let lines: Option<Vec<String>> = utils::get_input_content(working_dir, url, file_path, verbose);
    lines.map(|l| m3u_parser::parse_m3u(cfg, &l))
}


pub(crate) fn get_xtream_playlist(input: &ConfigInput, working_dir: &String, verbose: bool) -> Option<Vec<PlaylistGroup>> {
    let mut playlist: Vec<PlaylistGroup> = Vec::new();
    let base_url = format!("{}/player_api.php?username={}&password={}", input.url, input.username, input.password);
    let stream_base_url = format!("{}/{}/{}", input.url, input.username, input.password);

    let actions = [
        (XtreamCluster::LIVE, String::from("get_live_categories"), String::from("get_live_streams")),
        (XtreamCluster::VIDEO, String::from("get_vod_categories"), String::from("get_vod_streams")),
        (XtreamCluster::SERIES, String::from("get_series_categories"), String::from("get_series"))];
    for (xtream_cluster, category, stream) in &actions {
        let category_url =  format!("{}&action={}", base_url, category);
        let stream_url =  format!("{}&action={}", base_url, stream);
        let category_file_path = prepare_file_path(input, working_dir, format!("{}_", category).as_str(), verbose);
        let stream_file_path = prepare_file_path(input, working_dir, format!("{}_", stream).as_str(), verbose);

        let category_content: Option<serde_json::Value> = utils::get_input_json_content(input, &category_url, category_file_path, verbose);
        let stream_content: Option<serde_json::Value> = utils::get_input_json_content(input, &stream_url, stream_file_path, verbose);
        let mut sub_playlist = xtream_parser::parse_xtream(xtream_cluster, category_content, stream_content, &stream_base_url);
        while let Some(group) = sub_playlist.pop() {
            playlist.push(group);
        }
    }
    Some(playlist)
}
