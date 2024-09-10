use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use serde_json::Value;

use crate::create_m3u_filter_error_result;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::ConfigInput;
use crate::model::playlist::{PlaylistGroup, PlaylistItem, PlaylistItemHeader, PlaylistItemType, XtreamCluster};
use crate::model::xtream::{XtreamCategory, XtreamSeriesInfo, XtreamStream};
use crate::utils::default_utils::default_as_empty_rc_str;

fn map_to_xtream_category(category: &Value) -> Result<Vec<XtreamCategory>, M3uFilterError> {
    match serde_json::from_value::<Vec<XtreamCategory>>(category.to_owned()) {
        Ok(category_list) => Ok(category_list),
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

pub(crate) fn parse_xtream_series_info(info: &Value, group_title: &str, input: &ConfigInput) -> Result<Option<Vec<PlaylistItem>>, M3uFilterError> {
    let url = input.url.as_str();
    let username = input.username.as_ref().map_or("", |v| v);
    let password = input.password.as_ref().map_or("", |v| v);

    match serde_json::from_value::<XtreamSeriesInfo>(info.to_owned()) {
        Ok(series_info) => {
            let result: Vec<PlaylistItem> = series_info.episodes.values().flatten().map(|episode|
                PlaylistItem {
                    header: RefCell::new(PlaylistItemHeader {
                        uuid: default_as_empty_rc_str(),
                        id: Rc::new(episode.id.clone()),
                        stream_id: Rc::new(episode.id.clone()),
                        name: Rc::new(episode.title.clone()),
                        chno: default_as_empty_rc_str(),
                        logo: Rc::new(episode.info.movie_image.clone()),
                        logo_small: default_as_empty_rc_str(),
                        group: Rc::new(group_title.to_string()),
                        title: Rc::new(episode.title.clone()),
                        parent_code: default_as_empty_rc_str(),
                        audio_track: default_as_empty_rc_str(),
                        time_shift: default_as_empty_rc_str(),
                        rec: default_as_empty_rc_str(),
                        url: if episode.direct_source.is_empty() {
                            let ext = episode.container_extension.clone();
                            let stream_base_url = format!("{}/series/{}/{}/{}.{}", url, username, password, episode.id.as_str(), ext);
                            Rc::new(stream_base_url)
                        } else {
                            Rc::new(episode.direct_source.clone())
                        },
                        epg_channel_id: None,
                        item_type: PlaylistItemType::Series,
                        xtream_cluster: XtreamCluster::Series,
                        additional_properties: episode.get_additional_properties(&series_info),
                        series_fetched: false,
                        category_id: 0,
                        input_id: input.id,
                    })
                }).collect();
            if result.is_empty() { Ok(None) } else { Ok(Some(result)) }
        }
        Err(err) => {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Failed to process series info {}", &err)
        }
    }
}

pub(crate) fn parse_xtream(input: &ConfigInput,
                           xtream_cluster: XtreamCluster,
                           category: &Value,
                           streams: &Value) -> Result<Option<Vec<PlaylistGroup>>, M3uFilterError> {
    match map_to_xtream_category(category) {
        Ok(mut categories) => {
            let input_id = input.id;
            let url = input.url.as_str();
            let username = input.username.as_ref().map_or("", |v| v);
            let password = input.password.as_ref().map_or("", |v| v);

            return match map_to_xtream_streams(xtream_cluster, streams) {
                Ok(streams) => {
                    let group_map: HashMap::<Rc<String>, RefCell<XtreamCategory>> =
                        categories.drain(..).map(|category|
                            (Rc::clone(&category.category_id), RefCell::new(category))
                        ).collect();

                    for stream in streams {
                        if let Some(group) = group_map.get(&stream.category_id) {
                            let mut grp = group.borrow_mut();
                            let category_name = &grp.category_name;
                            let item = PlaylistItem {
                                header: RefCell::new(PlaylistItemHeader {
                                    uuid: default_as_empty_rc_str(),
                                    id: Rc::new(stream.get_stream_id()),
                                    stream_id: Rc::new(stream.get_stream_id()),
                                    name: Rc::clone(&stream.name),
                                    chno: default_as_empty_rc_str(),
                                    logo: Rc::clone(&stream.stream_icon),
                                    logo_small: default_as_empty_rc_str(),
                                    group: Rc::clone(category_name),
                                    title: Rc::clone(&stream.name),
                                    parent_code: default_as_empty_rc_str(),
                                    audio_track: default_as_empty_rc_str(),
                                    time_shift: default_as_empty_rc_str(),
                                    rec: default_as_empty_rc_str(),
                                    url: if stream.direct_source.is_empty() {
                                        let stream_base_url = match xtream_cluster {
                                            XtreamCluster::Live => format!("{}/live/{}/{}/{}.ts", url, username, password, &stream.get_stream_id()),
                                            XtreamCluster::Video => {
                                                let ext = stream.container_extension.as_ref().map_or("mp4", |e| e.as_str());
                                                format!("{}/movie/{}/{}/{}.{}", url, username, password, &stream.get_stream_id(), ext)
                                            }
                                            XtreamCluster::Series =>
                                                format!("{}/player_api.php?username={}&password={}&action=get_series_info&series_id={}",
                                                        url, username, password, &stream.get_stream_id())
                                        };
                                        Rc::new(stream_base_url)
                                    } else {
                                        Rc::clone(&stream.direct_source)
                                    },
                                    epg_channel_id: stream.epg_channel_id.clone(),
                                    item_type: match xtream_cluster {
                                        XtreamCluster::Live => PlaylistItemType::Live,
                                        XtreamCluster::Video => PlaylistItemType::Movie,
                                        XtreamCluster::Series => PlaylistItemType::SeriesInfo,
                                    },
                                    xtream_cluster,
                                    additional_properties: stream.get_additional_properties(),
                                    series_fetched: false,
                                    category_id: 0,
                                    input_id,
                                }),
                            };
                            grp.add(item);
                        }
                    }

                    Ok(Some(group_map.values().map(|category| {
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
            };
        }
        Err(err) => Err(err)
    }
}