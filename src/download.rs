use std::path::PathBuf;
use std::sync::atomic::AtomicI32;
use log::debug;
use crate::{utils};
use crate::m3u_filter_error::M3uFilterError;
use crate::model::config::{Config, ConfigInput};
use crate::model::model_m3u::{PlaylistGroup, XtreamCluster};
use crate::model::xmltv::TVGuide;
use crate::processing::{m3u_parser, xmltv_parser, xtream_parser};
use crate::utils::add_prefix_to_filename;

fn prepare_file_path(input: &ConfigInput, working_dir: &String, action: &str) -> Option<PathBuf> {
    let persist_file: Option<PathBuf> =
        match &input.persist {
            Some(persist_path) => utils::prepare_persist_path(persist_path.as_str(), action),
            _ => None
        };
    if persist_file.is_some() {
        let file_path = utils::get_file_path(working_dir, persist_file);
        debug!("persist to file:  {:?}", match &file_path {
            Some(fp) => fp.display().to_string(),
            _ => "".to_string()
        });
        file_path
    } else {
        None
    }
}

pub(crate) fn get_m3u_playlist(cfg: &Config, input: &ConfigInput, working_dir: &String) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {
    let url = input.url.to_owned();
    let persist_file_path = prepare_file_path(input, working_dir, "");
    match utils::get_input_text_content(input, working_dir,&url, persist_file_path) {
        Ok(text) => {
            let lines = text.lines().map(String::from).collect();
            (m3u_parser::parse_m3u(cfg, &lines), vec![])
        }
        Err(err) => (vec![], vec![err])
    }
}

const ACTIONS: [(XtreamCluster, &str, &str); 3] = [
    (XtreamCluster::Live, "get_live_categories", "get_live_streams"),
    (XtreamCluster::Video, "get_vod_categories", "get_vod_streams"),
    (XtreamCluster::Series, "get_series_categories", "get_series")];

pub(crate) fn get_xtream_playlist(input: &ConfigInput, working_dir: &String) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {
    let mut playlist: Vec<PlaylistGroup> = Vec::new();
    let username = input.username.as_ref().unwrap().clone();
    let password = input.password.as_ref().unwrap().clone();
    let base_url = format!("{}/player_api.php?username={}&password={}", input.url, username, password);
    let stream_base_url = format!("{}/{}/{}", input.url, username, password);

    let mut errors = vec![];
    let category_id_cnt = AtomicI32::new(0);
    for (xtream_cluster, category, stream) in &ACTIONS {
        let category_url = format!("{}&action={}", base_url, category);
        let stream_url = format!("{}&action={}", base_url, stream);
        let category_file_path = prepare_file_path(input, working_dir, format!("{}_", category).as_str());
        let stream_file_path = prepare_file_path(input, working_dir, format!("{}_", stream).as_str());

        match utils::get_input_json_content(input, &category_url, category_file_path) {
            Ok(category_content) => {
                match utils::get_input_json_content(input, &stream_url, stream_file_path) {
                    Ok(stream_content) => {
                        match xtream_parser::parse_xtream(&category_id_cnt, xtream_cluster, &category_content, &stream_content, &stream_base_url) {
                            Ok(sub_playlist_opt) => {
                                if let Some(mut sub_playlist) = sub_playlist_opt {
                                    sub_playlist.drain(..).for_each(|group| playlist.push(group));
                                }
                            }
                            Err(err) => errors.push(err)
                        }
                    }
                    Err(err) => errors.push(err)
                }
            }
            Err(err) => errors.push(err)
        }
    }
    (playlist, errors)
}


pub(crate) fn get_xmltv(_cfg: &Config, input: &ConfigInput, working_dir: &String) -> (Option<TVGuide>, Vec<M3uFilterError>) {
    match &input.epg_url {
        None => (None, vec![]),
        Some(url) => {
            debug!("Getting epg file path for url: {}", url);
            let persist_file_path = prepare_file_path(input, working_dir, "").map(|path| add_prefix_to_filename(&path, "epg_", Some("xml")));
            match utils::get_input_text_content(input, working_dir, url, persist_file_path) {
                Ok(xml_content) => {
                    xmltv_parser::parse_tvguide(&xml_content)
                }
                Err(err) => (None, vec![err])
            }

        }
    }
}