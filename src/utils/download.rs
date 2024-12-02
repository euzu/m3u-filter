use std::cmp::Ordering;
use std::path::PathBuf;
use std::thread::sleep;
use log::{debug, info};
use crate::m3u_filter_error::M3uFilterError;
use crate::model::config::{Config, ConfigInput};
use crate::model::playlist::{FetchedPlaylist, PlaylistGroup, PlaylistItem, PlaylistItemType, XtreamCluster};
use crate::model::xmltv::TVGuide;
use crate::processing::{m3u_parser, xtream_parser};
use crate::processing::xtream_parser::parse_xtream_series_info;
use crate::utils::{file_utils, request_utils};

fn prepare_file_path(persist: Option<&String>, working_dir: &str, action: &str) -> Option<PathBuf> {
    let persist_file: Option<PathBuf> =
        persist.map(|persist_path| file_utils::prepare_persist_path(persist_path.as_str(), action));
    if persist_file.is_some() {
        let file_path = file_utils::get_file_path(working_dir, persist_file);
        debug!("persist to file:  {file_path:?}");
        file_path
    } else {
        None
    }
}

pub async fn get_m3u_playlist(cfg: &Config, input: &ConfigInput, working_dir: &String) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {
    let url = input.url.clone();
    let persist_file_path = prepare_file_path(input.persist.as_ref(), working_dir, "");
    match request_utils::get_input_text_content(input, working_dir, &url, persist_file_path).await {
        Ok(text) => {
            (m3u_parser::parse_m3u(cfg, input, text.lines()), vec![])
        }
        Err(err) => (vec![], vec![err])
    }
}

pub async fn get_xtream_playlist_series(fpl: &mut FetchedPlaylist<'_>, errors: &mut Vec<M3uFilterError>, resolve_delay: u16) -> Vec<PlaylistGroup> {
    let input = fpl.input;
    let mut result: Vec<PlaylistGroup> = vec![];
    for plg in &mut fpl.playlistgroups {
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
                                    group_series.append(&mut series);
                                }
                            }
                            Err(err) => errors.push(err),
                        }
                    }
                    Err(err) => errors.push(err)
                };
                if resolve_delay > 0 {
                    sleep(std::time::Duration::new(u64::from(resolve_delay), 0)); // 2 seconds between
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

fn get_skip_cluster(input: &ConfigInput) -> Vec<XtreamCluster> {
    let mut skip_cluster = vec![];
    if let Some(input_options) = &input.options {
        if input_options.xtream_skip_live {
            skip_cluster.push(XtreamCluster::Live);
        }
        if input_options.xtream_skip_vod {
            skip_cluster.push(XtreamCluster::Video);
        }
        if input_options.xtream_skip_series {
            skip_cluster.push(XtreamCluster::Series);
        }
    }
    if skip_cluster.len() == 3 {
        let name = input.name.as_ref().map_or_else(|| input.id.to_string(), std::string::ToString::to_string);
        info!("You have skipped all sections from xtream input {name}");
    }
    skip_cluster
}

const ACTIONS: [(XtreamCluster, &str, &str); 3] = [
    (XtreamCluster::Live, "get_live_categories", "get_live_streams"),
    (XtreamCluster::Video, "get_vod_categories", "get_vod_streams"),
    (XtreamCluster::Series, "get_series_categories", "get_series")];

pub async fn get_xtream_playlist(input: &ConfigInput, working_dir: &str) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {
    let mut playlist_groups: Vec<PlaylistGroup> = Vec::new();
    let username = input.username.as_ref().map_or("", |v| v);
    let password = input.password.as_ref().map_or("", |v| v);
    let base_url = format!("{}/player_api.php?username={}&password={}", input.url, username, password);

    let skip_cluster = get_skip_cluster(input);

    let mut errors = vec![];
    for (xtream_cluster, category, stream) in &ACTIONS {
        if !skip_cluster.contains(xtream_cluster) {
            let category_url = format!("{base_url}&action={category}");
            let stream_url = format!("{base_url}&action={stream}");
            let category_file_path = prepare_file_path(input.persist.as_ref(), working_dir, format!("{category}_").as_str());
            let stream_file_path = prepare_file_path(input.persist.as_ref(), working_dir, format!("{stream}_").as_str());

            match futures::join!(
                request_utils::get_input_json_content(input, category_url.as_str(), category_file_path),
                request_utils::get_input_json_content(input, stream_url.as_str(), stream_file_path)
            ) {
                (Ok(category_content), Ok(stream_content)) => {
                    match xtream_parser::parse_xtream(input,
                                                      *xtream_cluster,
                                                      &category_content,
                                                      &stream_content) {
                        Ok(sub_playlist_parsed) => {
                            if let Some(mut xtream_sub_playlist) = sub_playlist_parsed {
                                playlist_groups.append(&mut xtream_sub_playlist);
                            }
                        }
                        Err(err) => errors.push(err)
                    }
                },
                (Err(err1), Err(err2)) => {
                    errors.extend([err1, err2]);
                },
                (_, Err(err)) | (Err(err), _) => errors.push(err),
            }
        }
    }
    playlist_groups.sort_by(|a, b| a.title.partial_cmp(&b.title).unwrap_or(Ordering::Greater));

    for (grp_id, plg) in (1_u32..).zip(playlist_groups.iter_mut()) {
        plg.id = grp_id;
    }
    (playlist_groups, errors)
}

pub async fn get_xmltv(_cfg: &Config, input: &ConfigInput, working_dir: &str) -> (Option<TVGuide>, Vec<M3uFilterError>) {
    match &input.epg_url {
        None => (None, vec![]),
        Some(url) => {
            debug!("Getting epg file path for url: {}", url);
            let persist_file_path = prepare_file_path(input.persist.as_ref(), working_dir, "")
                .map(|path| file_utils::add_prefix_to_filename(&path, "epg_", Some("xml")));

            match request_utils::get_input_text_content_as_file(input, working_dir, url, persist_file_path).await {
                Ok(file) => {
                    (Some(TVGuide { file }), vec![])
                }
                Err(err) => (None, vec![err])
            }
        }
    }
}