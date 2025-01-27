use crate::Arc;
use std::borrow::Cow;
use crate::m3u_filter_error::{str_to_io_error, M3uFilterError};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::playlist::{PlaylistEntry, PlaylistGroup, XtreamCluster};
use crate::model::xmltv::TVGuide;
use crate::processing::{m3u_parser, xtream_parser};
use crate::repository::xtream_repository::{rewrite_xtream_series_info_content, rewrite_xtream_vod_info_content, xtream_get_input_info};
use crate::repository::xtream_repository;
use crate::utils::{file_utils, request_utils};
use log::{debug, info};
use std::cmp::Ordering;
use std::io::{Error};
use std::path::PathBuf;
use crate::debug_if_enabled;
use crate::model::api_proxy::{ProxyUserCredentials};

const ACTION_GET_SERIES_INFO: &str = "get_series_info";
const ACTION_GET_VOD_INFO: &str = "get_vod_info";
const ACTION_GET_LIVE_INFO: &str = "get_live_info";

fn prepare_file_path(persist: Option<&str>, working_dir: &str, action: &str) -> Option<PathBuf> {
    let persist_file: Option<PathBuf> =
        persist.map(|persist_path| file_utils::prepare_persist_path(persist_path, action));
    if persist_file.is_some() {
        let file_path = file_utils::get_file_path(working_dir, persist_file);
        debug_if_enabled!("persist to file:  {}", file_path.as_ref().map_or(Cow::from("?"), |p| p.to_string_lossy()));
        file_path
    } else {
        None
    }
}

pub async fn get_m3u_playlist(client: Arc<reqwest::Client>, cfg: &Config, input: &ConfigInput, working_dir: &str) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {
    let url = input.url.clone();
    let persist_file_path = prepare_file_path(input.persist.as_deref(), working_dir, "");
    match request_utils::get_input_text_content(client, input, working_dir, &url, persist_file_path).await {
        Ok(text) => {
            (m3u_parser::parse_m3u(cfg, input, text.lines()), vec![])
        }
        Err(err) => (vec![], vec![err])
    }
}

pub fn get_xtream_player_api_action_url(input: &ConfigInput, action: &str) -> Option<String> {
    if let Some(user_info) = input.get_user_info() {
        Some(format!("{}/player_api.php?username={}&password={}&action={}",
                     &user_info.base_url,
                     &user_info.username,
                     &user_info.password,
                     action
        ))
    } else {
        None
    }
}

pub fn get_xtream_player_api_info_url(input: &ConfigInput, cluster: XtreamCluster, stream_id: u32) -> Option<String> {
    let (action, stream_id_field) = match cluster {
        XtreamCluster::Live => (ACTION_GET_LIVE_INFO, "live_id"),
        XtreamCluster::Video => (ACTION_GET_VOD_INFO, "vod_id"),
        XtreamCluster::Series => (ACTION_GET_SERIES_INFO, "series_id"),
    };
    get_xtream_player_api_action_url(input, action).map(|action_url| format!("{action_url}&{stream_id_field}={stream_id}"))
}


pub async fn get_xtream_stream_info_content(client: Arc<reqwest::Client>, info_url: &str, input: &ConfigInput) -> Result<String, Error> {
    request_utils::download_text_content(client, input, info_url, None).await
}

#[allow(clippy::too_many_arguments)]
pub async fn get_xtream_stream_info<P>(client: Arc<reqwest::Client>,
                                       config: &Config,
                                       user: &ProxyUserCredentials,
                                       input: &ConfigInput,
                                       target: &ConfigTarget,
                                       pli: &P,
                                       info_url: &str,
                                       cluster: XtreamCluster) -> Result<String, Error>
where
    P: PlaylistEntry,
{
    if cluster == XtreamCluster::Series {
        if let Some(content) = xtream_repository::xtream_load_series_info(config, target.name.as_str(), pli.get_virtual_id()).await {
            // Deliver existing target content
            return rewrite_xtream_series_info_content(config, target, pli, user, &content).await;
        }

        // Check if the content has been resolved
        let resolve_series = target.options.as_ref().is_some_and(|opt| opt.xtream_resolve_series);
        if resolve_series {
            if let Some(provider_id) = pli.get_provider_id() {
                if let Some(content) = xtream_get_input_info(config, input, provider_id, XtreamCluster::Series).await {
                    return xtream_repository::write_and_get_xtream_series_info(config, target, pli, user, &content).await;
                }
            }
        }
    } else if cluster == XtreamCluster::Video {
        if let Some(content) = xtream_repository::xtream_load_vod_info(config, target.name.as_str(), pli.get_virtual_id()).await {
            // Deliver existing target content
            return rewrite_xtream_vod_info_content(config, target, pli, user, &content);
        }
        // Check if the content has been resolved
        let resolve_vod = target.options.as_ref().is_some_and(|opt| opt.xtream_resolve_vod);
        if resolve_vod {
            if let Some(provider_id) = pli.get_provider_id() {
                if let Some(content) = xtream_get_input_info(config, input, provider_id, XtreamCluster::Video).await {
                    return xtream_repository::write_and_get_xtream_vod_info(config, target, pli, user, &content).await;
                }
            }
        }
    }

    if let Ok(content) = get_xtream_stream_info_content(client, info_url, input).await {
        return match cluster {
            XtreamCluster::Live => Ok(content),
            XtreamCluster::Video => xtream_repository::write_and_get_xtream_vod_info(config, target, pli, user, &content).await,
            XtreamCluster::Series => xtream_repository::write_and_get_xtream_series_info(config, target, pli, user, &content).await,
        };
    }

    Err(str_to_io_error(&format!("Cant find stream with id: {}/{}/{}",
                                             target.name.replace(' ', "_").as_str(), &cluster, pli.get_virtual_id())))
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
        info!("You have skipped all sections from xtream input {}", &input.name);
    }
    skip_cluster
}

const ACTIONS: [(XtreamCluster, &str, &str); 3] = [
    (XtreamCluster::Live, "get_live_categories", "get_live_streams"),
    (XtreamCluster::Video, "get_vod_categories", "get_vod_streams"),
    (XtreamCluster::Series, "get_series_categories", "get_series")];

pub async fn get_xtream_playlist(client: Arc<reqwest::Client>, input: &ConfigInput, working_dir: &str) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {
    let mut playlist_groups: Vec<PlaylistGroup> = Vec::with_capacity(128);
    let username = input.username.as_ref().map_or("", |v| v);
    let password = input.password.as_ref().map_or("", |v| v);
    let base_url = format!("{}/player_api.php?username={}&password={}", input.url, username, password);

    let skip_cluster = get_skip_cluster(input);

    let mut errors = vec![];
    for (xtream_cluster, category, stream) in &ACTIONS {
        if !skip_cluster.contains(xtream_cluster) {
            let category_url = format!("{base_url}&action={category}");
            let stream_url = format!("{base_url}&action={stream}");
            let category_file_path = prepare_file_path(input.persist.as_deref(), working_dir, format!("{category}_").as_str());
            let stream_file_path = prepare_file_path(input.persist.as_deref(), working_dir, format!("{stream}_").as_str());

            match futures::join!(
                request_utils::get_input_json_content(Arc::clone(&client), input, category_url.as_str(), category_file_path),
                request_utils::get_input_json_content(Arc::clone(&client), input, stream_url.as_str(), stream_file_path)
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
                }
                (Err(err1), Err(err2)) => {
                    errors.extend([err1, err2]);
                }
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

pub async fn get_xmltv(client: Arc<reqwest::Client>, _cfg: &Config, input: &ConfigInput, working_dir: &str) -> (Option<TVGuide>, Vec<M3uFilterError>) {
    match &input.epg_url {
        None => (None, vec![]),
        Some(url) => {
            debug!("Getting epg file path for url: {}", url);
            let persist_file_path = prepare_file_path(input.persist.as_deref(), working_dir, "")
                .map(|path| file_utils::add_prefix_to_filename(&path, "epg_", Some("xml")));

            match request_utils::get_input_text_content_as_file(client, input, working_dir, url, persist_file_path).await {
                Ok(file) => {
                    (Some(TVGuide { file }), vec![])
                }
                Err(err) => (None, vec![err])
            }
        }
    }
}