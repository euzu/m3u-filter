use crate::Arc;
use crate::m3u_filter_error::{str_to_io_error, M3uFilterError};
use crate::model::config::{Config, ConfigInput, ConfigTarget};
use crate::model::playlist::{PlaylistEntry, PlaylistGroup, XtreamCluster};
use crate::processing::{xtream_parser};
use crate::repository::xtream_repository::{rewrite_xtream_series_info_content, rewrite_xtream_vod_info_content, xtream_get_input_info};
use crate::repository::xtream_repository;
use crate::utils::{request_utils};
use log::{info, warn};
use std::cmp::Ordering;
use std::io::{Error};
use crate::model::api_proxy::{ProxyUserCredentials};
pub const ACTION_GET_SERIES_INFO: &str = "get_series_info";
pub const ACTION_GET_VOD_INFO: &str = "get_vod_info";
pub const ACTION_GET_LIVE_INFO: &str = "get_live_info";
pub const ACTION_GET_SERIES: &str = "get_series";
pub const ACTION_GET_LIVE_CATEGORIES: &str = "get_live_categories";
pub const ACTION_GET_VOD_CATEGORIES: &str = "get_vod_categories";
pub const ACTION_GET_SERIES_CATEGORIES: &str = "get_series_categories";
pub const ACTION_GET_LIVE_STREAMS: &str = "get_live_streams";
pub const ACTION_GET_VOD_STREAMS: &str = "get_vod_streams";

#[inline]
pub fn get_xtream_stream_url_base(url: &str, username: &str, password: &str) -> String {
    format!("{url}/player_api.php?username={username}&password={password}")
}

pub fn get_xtream_player_api_action_url(input: &ConfigInput, action: &str) -> Option<String> {
    if let Some(user_info) = input.get_user_info() {
        Some(format!("{}&action={}",
                     get_xtream_stream_url_base(
                         &user_info.base_url,
                         &user_info.username,
                         &user_info.password),
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
    (XtreamCluster::Live, ACTION_GET_LIVE_CATEGORIES, ACTION_GET_LIVE_STREAMS),
    (XtreamCluster::Video, ACTION_GET_VOD_CATEGORIES, ACTION_GET_VOD_STREAMS),
    (XtreamCluster::Series, ACTION_GET_SERIES_CATEGORIES, ACTION_GET_SERIES)];

pub async fn get_xtream_playlist(client: Arc<reqwest::Client>, input: &ConfigInput, working_dir: &str) -> (Vec<PlaylistGroup>, Vec<M3uFilterError>) {

    let username = input.username.as_ref().map_or("", |v| v);
    let password = input.password.as_ref().map_or("", |v| v);
    let base_url = format!("{}/player_api.php?username={}&password={}", input.url, username, password);

    if let Err(err) = request_utils::get_input_json_content(Arc::clone(&client), input, base_url.as_str(), None).await {
        warn!("Failed to login xtream account {username} {err}");
        return (Vec::with_capacity(0), vec![err]);
    };


    let mut playlist_groups: Vec<PlaylistGroup> = Vec::with_capacity(128);
    let skip_cluster = get_skip_cluster(input);

    let mut errors = vec![];
    for (xtream_cluster, category, stream) in &ACTIONS {
        if !skip_cluster.contains(xtream_cluster) {
            let category_url = format!("{base_url}&action={category}");
            let stream_url = format!("{base_url}&action={stream}");
            let category_file_path = crate::utils::download::prepare_file_path(input.persist.as_deref(), working_dir, format!("{category}_").as_str());
            let stream_file_path = crate::utils::download::prepare_file_path(input.persist.as_deref(), working_dir, format!("{stream}_").as_str());

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