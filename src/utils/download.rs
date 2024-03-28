use std::path::PathBuf;
use std::thread::sleep;
use log::{debug, info};
use crate::m3u_filter_error::M3uFilterError;
use crate::model::config::{Config, ConfigInput};
use crate::model::playlist::{FetchedPlaylist, PlaylistGroup, PlaylistItem, PlaylistItemType, XtreamCluster};
use crate::model::xmltv::TVGuide;
use crate::processing::{m3u_parser, xmltv_parser, xtream_parser};
use crate::processing::xtream_parser::parse_xtream_series_info;
use crate::utils::{file_utils, request_utils};

fn prepare_file_path(input: &ConfigInput, working_dir: &String, action: &str) -> Option<PathBuf> {
    let persist_file: Option<PathBuf> =
        match &input.persist {
            Some(persist_path) => file_utils::prepare_persist_path(persist_path.as_str(), action),
            _ => None
        };
    if persist_file.is_some() {
        let file_path = file_utils::get_file_path(working_dir, persist_file);
        debug!("persist to file:  {:?}", match &file_path {
            Some(fp) => fp.display().to_string(),
            _ => "".to_string()
        });
        file_path
    } else {
        None
    }
}

pub(crate) async fn get_m3u_playlist(cfg: &Config, input: &ConfigInput, working_dir: &String) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {
    let url = input.url.to_owned();
    let persist_file_path = prepare_file_path(input, working_dir, "");
    match request_utils::get_input_text_content(input, working_dir, &url, persist_file_path).await {
        Ok(text) => {
            let lines = text.lines().map(String::from).collect();
            (m3u_parser::parse_m3u(cfg, &lines), vec![])
        }
        Err(err) => (vec![], vec![err])
    }
}

pub(crate) async fn get_xtream_playlist_series<'a>(fpl: &mut FetchedPlaylist<'a>, errors: &mut Vec<M3uFilterError>, resolve_delay: u16) -> Vec<PlaylistGroup> {
    let input = fpl.input;
    let mut result: Vec<PlaylistGroup> = vec![];
    for plg in &mut fpl.playlist {
        let mut group_series: Vec<PlaylistItem> = vec![];
        for pli in &plg.channels {
            let (fetch_series, series_info_url) = {
                let mut header = pli.header.borrow_mut();
                let fetch_series = !header.series_fetched && header.item_type == PlaylistItemType::SeriesInfo;
                if fetch_series {
                    header.series_fetched = true;
                }
                (fetch_series, header.url.to_string())
            };
            if fetch_series {
                match request_utils::get_input_json_content(fpl.input, series_info_url.as_str(), None).await {
                    Ok(series_content) => {
                        match parse_xtream_series_info(&series_content, pli.header.borrow().group.as_str(), input) {
                            Ok(series_info) => {
                                if let Some(mut series) = series_info {
                                    series.drain(..).for_each(|item| group_series.push(item));
                                }
                            }
                            Err(err) => errors.push(err),
                        }
                    }
                    Err(err) => errors.push(err)
                };
                if resolve_delay > 0 {
                    sleep(std::time::Duration::new(resolve_delay as u64, 0)); // 2 seconds between
                }
            }
        }
        if !group_series.is_empty() {
            let group = PlaylistGroup {
                id: plg.id,
                title: plg.title.clone(),
                channels: group_series,
                xtream_cluster: XtreamCluster::Series,
            };
            result.push(group);
        }
    }
    result
}

fn get_skip_cluster(input: &&ConfigInput) -> Vec<XtreamCluster> {
    let mut skip_cluster = vec![];
    if let Some(input_options) = &input.options {
        if input_options.xtream_skip_live {
            skip_cluster.push(XtreamCluster::Live)
        }
        if input_options.xtream_skip_vod {
            skip_cluster.push(XtreamCluster::Video)
        }
        if input_options.xtream_skip_series {
            skip_cluster.push(XtreamCluster::Series)
        }
    }
    if skip_cluster.len() == 3 {
        info!("You have skipped all sections from xtream input {}", input.name.as_ref().unwrap_or(&input.id.to_string()));
    }
    skip_cluster
}

const ACTIONS: [(XtreamCluster, &str, &str); 3] = [
    (XtreamCluster::Live, "get_live_categories", "get_live_streams"),
    (XtreamCluster::Video, "get_vod_categories", "get_vod_streams"),
    (XtreamCluster::Series, "get_series_categories", "get_series")];

pub(crate) async fn get_xtream_playlist(input: &ConfigInput, working_dir: &String) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {
    let mut playlist: Vec<PlaylistGroup> = Vec::new();
    let username = input.username.as_ref().map_or("", |v| v);
    let password = input.password.as_ref().map_or("", |v| v);
    let base_url = format!("{}/player_api.php?username={}&password={}", input.url, username, password);

    let skip_cluster = get_skip_cluster(&input);

    let mut errors = vec![];
    for (xtream_cluster, category, stream) in &ACTIONS {
        if !skip_cluster.contains(xtream_cluster) {
            let category_url = format!("{}&action={}", base_url, category);
            let stream_url = format!("{}&action={}", base_url, stream);
            let category_file_path = prepare_file_path(input, working_dir, format!("{}_", category).as_str());
            let stream_file_path = prepare_file_path(input, working_dir, format!("{}_", stream).as_str());

            match request_utils::get_input_json_content(input, category_url.as_str(), category_file_path).await {
                Ok(category_content) => {
                    match request_utils::get_input_json_content(input, stream_url.as_str(), stream_file_path).await {
                        Ok(stream_content) => {
                            match xtream_parser::parse_xtream(input,
                                                              xtream_cluster,
                                                              &category_content,
                                                              &stream_content) {
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
    }
    (playlist, errors)
}


pub(crate) async fn get_xmltv(_cfg: &Config, input: &ConfigInput, working_dir: &String) -> (Option<TVGuide>, Vec<M3uFilterError>) {
    match &input.epg_url {
        None => (None, vec![]),
        Some(url) => {
            debug!("Getting epg file path for url: {}", url);
            let persist_file_path = prepare_file_path(input, working_dir, "").map(|path| file_utils::add_prefix_to_filename(&path, "epg_", Some("xml")));
            match request_utils::get_input_text_content(input, working_dir, url, persist_file_path).await {
                Ok(xml_content) => {
                    (xmltv_parser::parse_tvguide(xml_content.as_str()), vec![])
                }
                Err(err) => (None, vec![err])
            }
        }
    }
}