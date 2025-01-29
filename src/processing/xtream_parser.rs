use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use serde_json::Value;

use crate::create_m3u_filter_error_result;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::ConfigInput;
use crate::model::playlist::{PlaylistGroup, PlaylistItem, PlaylistItemHeader, PlaylistItemType, XtreamCluster};
use crate::model::xtream::{XtreamCategory, XtreamSeriesInfo, XtreamSeriesInfoEpisode, XtreamStream};
use crate::utils::hash_utils::generate_playlist_uuid;
use crate::utils::xtream_utils::{get_xtream_stream_url_base, ACTION_GET_SERIES_INFO};

fn map_to_xtream_category(categories: &Value) -> Result<Vec<XtreamCategory>, M3uFilterError> {
    match serde_json::from_value::<Vec<XtreamCategory>>(categories.to_owned()) {
        Ok(xtream_categories) => Ok(xtream_categories),
        Err(err) => {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Failed to process categories {}", &err)
        }
    }
}

fn map_to_xtream_streams(xtream_cluster: XtreamCluster, streams: &Value) -> Result<Vec<XtreamStream>, M3uFilterError> {
    match serde_json::from_value::<Vec<XtreamStream>>(streams.to_owned()) {
        Ok(stream_list) => Ok(stream_list),
        Err(err) => {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Failed to map to xtream streams {:?}: {}", xtream_cluster, &err)
        }
    }
}

fn create_xtream_series_episode_url(url: &str, username: &str, password: &str, episode: &XtreamSeriesInfoEpisode) -> Rc<String> {
    if episode.direct_source.is_empty() {
        let ext = episode.container_extension.clone();
        let stream_base_url = format!("{url}/series/{username}/{password}/{}.{ext}", episode.id);
        Rc::new(stream_base_url)
    } else {
        Rc::new(episode.direct_source.clone())
    }
}

pub fn parse_xtream_series_info(info: &Value, group_title: &str, series_name: &str, input: &ConfigInput) -> Result<Option<Vec<(XtreamSeriesInfoEpisode, PlaylistItem)>>, M3uFilterError> {
    let url = input.url.as_str();
    let username = input.username.as_ref().map_or("", |v| v);
    let password = input.password.as_ref().map_or("", |v| v);

    match serde_json::from_value::<XtreamSeriesInfo>(info.to_owned()) {
        Ok(series_info) => {
            if let Some(episodes) = &series_info.episodes {
                let result: Vec<(XtreamSeriesInfoEpisode, PlaylistItem)> = episodes.values().flatten().map(|episode| {
                    let episode_url = create_xtream_series_episode_url(url, username, password, episode);
                    (episode.clone(),
                     PlaylistItem {
                         header: RefCell::new(PlaylistItemHeader {
                             id: Rc::new(episode.id.to_string()),
                             uuid: Rc::new(generate_playlist_uuid(&input.name, &episode.id, &episode_url)),
                             name: Rc::new(series_name.to_string()),
                             logo: Rc::new(episode.info.as_ref().map_or_else(String::new, |info| info.movie_image.to_string())),
                             group: Rc::new(group_title.to_string()),
                             title: Rc::new(episode.title.clone()),
                             url: episode_url,
                             item_type: PlaylistItemType::Series,
                             xtream_cluster: XtreamCluster::Series,
                             additional_properties: episode.get_additional_properties(&series_info),
                             category_id: 0,
                             input_name: Rc::new(input.name.to_string()),
                             ..Default::default()
                         })
                     })
                }).collect();
                return if result.is_empty() { Ok(None) } else { Ok(Some(result)) };
            }
            Ok(None)
        }
        Err(err) => {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Failed to process series info for {series_name} {err}")
        }
    }
}

fn create_xtream_url(xtream_cluster: XtreamCluster, url: &str, username: &str, password: &str,
                     stream: &XtreamStream, live_stream_without_extension: bool) -> Rc<String> {
    if stream.direct_source.is_empty() {
        let stream_base_url = match xtream_cluster {
            XtreamCluster::Live => format!("{url}/live/{username}/{password}/{}{}", &stream.get_stream_id(), if live_stream_without_extension { "" } else { ".ts" }),
            XtreamCluster::Video => {
                let ext = stream.container_extension.as_ref().map_or("mp4", |e| e.as_str());
                format!("{url}/movie/{username}/{password}/{}.{ext}", &stream.get_stream_id())
            }
            XtreamCluster::Series =>
                format!("{}&action={ACTION_GET_SERIES_INFO}&series_id={}", get_xtream_stream_url_base(url, username, password), &stream.get_stream_id())
        };
        Rc::new(stream_base_url)
    } else {
        Rc::clone(&stream.direct_source)
    }
}

pub fn parse_xtream(input: &ConfigInput,
                    xtream_cluster: XtreamCluster,
                    categories: &Value,
                    streams: &Value) -> Result<Option<Vec<PlaylistGroup>>, M3uFilterError> {
    match map_to_xtream_category(categories) {
        Ok(xtream_categories) => {
            let input_name = Rc::new(input.name.to_string());
            let url = input.url.as_str();
            let username = input.username.as_ref().map_or("", |v| v);
            let password = input.password.as_ref().map_or("", |v| v);

            match map_to_xtream_streams(xtream_cluster, streams) {
                Ok(xtream_streams) => {
                    let mut group_map: HashMap::<Rc<String>, RefCell<XtreamCategory>> =
                        xtream_categories.into_iter().map(|category|
                            (Rc::clone(&category.category_id), RefCell::new(category))
                        ).collect();
                    let unknown_grp = RefCell::new(XtreamCategory {
                        category_id: Rc::new("0".to_string()),
                        category_name: Rc::new("Unknown".to_string()),
                        channels: vec![],
                    });

                    let live_stream_without_extension = input.options.as_ref().is_some_and(|o| o.xtream_live_stream_without_extension);

                    for stream in xtream_streams {
                        let group = group_map.get(&stream.category_id).unwrap_or(&unknown_grp);
                        let mut grp = group.borrow_mut();
                        let category_name = &grp.category_name;
                        let stream_url = create_xtream_url(xtream_cluster, url, username, password, &stream, live_stream_without_extension);
                        let item = PlaylistItem {
                            header: RefCell::new(PlaylistItemHeader {
                                id: Rc::new(stream.get_stream_id().to_string()),
                                uuid: Rc::new(generate_playlist_uuid(&input_name,  &stream.get_stream_id().to_string(), &stream_url)),
                                name: Rc::clone(&stream.name),
                                logo: Rc::clone(&stream.stream_icon),
                                group: Rc::clone(category_name),
                                title: Rc::clone(&stream.name),
                                url: stream_url,
                                epg_channel_id: stream.epg_channel_id.clone(),
                                item_type: PlaylistItemType::from(xtream_cluster),
                                xtream_cluster,
                                additional_properties: stream.get_additional_properties(),
                                category_id: 0,
                                input_name: Rc::clone(&input_name),
                                ..Default::default()
                            }),
                        };
                        grp.add(item);
                    }
                    let has_channels = !unknown_grp.borrow().channels.is_empty();
                    if has_channels {
                        group_map.insert(Rc::new("0".to_string()), unknown_grp);
                    }

                    Ok(Some(group_map.values().filter(|category| !category.borrow().channels.is_empty())
                        .map(|category| {
                            let cat = category.borrow();
                            PlaylistGroup {
                                id: cat.category_id.parse::<u32>().unwrap_or(0),
                                xtream_cluster,
                                title: Rc::clone(&cat.category_name),
                                channels: cat.channels.clone(),
                            }
                        }).collect()))
                }
                Err(err) => Err(err)
            }
        }
        Err(err) => Err(err)
    }
}

