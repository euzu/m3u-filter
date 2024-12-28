use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget};
use crate::model::playlist::{PlaylistGroup, PlaylistItem, PlaylistItemType};
use crate::model::xtream::XtreamSeriesEpisode;
use crate::repository::bplustree::BPlusTree;
use crate::repository::storage::get_input_storage_path;
use crate::repository::xtream_repository::{xtream_get_record_file_path, InputVodInfoRecord};
use crate::utils::file_lock_manager::FileReadGuard;
use crate::utils::file_utils;
use crate::{create_m3u_filter_error_result, notify_err};
use chrono::Datelike;
use log::error;
use regex::Regex;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::LazyLock;

struct KodiStyle {
    year: Regex,
    season: Regex,
    episode: Regex,
    whitespace: Regex,
    alphanumeric: Regex,
}

static KODI_STYLE: LazyLock<KodiStyle> = LazyLock::new(|| KodiStyle {
    season: regex::Regex::new(r"[Ss]\d{1,2}").unwrap(),
    episode: regex::Regex::new(r"[Ee]\d{1,2}").unwrap(),
    year: regex::Regex::new(r"\d{4}").unwrap(),
    whitespace: regex::Regex::new(r"\s+").unwrap(),
    alphanumeric: regex::Regex::new(r"[^\w\s]").unwrap(),
});

fn sanitize_for_filename(text: &str, underscore_whitespace: bool) -> String {
    text.trim().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || (*c == '(') || (*c == ')'))
        .map(|c| if underscore_whitespace { if c.is_whitespace() { '_' } else { c } } else { c })
        .collect::<String>()
}

fn extract_match(name: &str, pattern: &Regex) -> (String, Option<String>) {
    pattern.find(&name).map_or_else(|| (name.to_string(), None), |m| {
        let matched = String::from(&name[m.start()..m.end()]);
        let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
        (new_name, Some(matched))
    })
}

fn extract_season_or_episode_match_with_default(name: &str, pattern: &Regex, default_value: Option<&String>) -> (String, Option<u32>) {
    let (new_name, value) = extract_match(name, pattern);
    let new_value = match value {
        // we skip the prefix S or E to parse the num
        Some(num_value) => num_value[1..].parse::<u32>().ok(),
        None => default_value.and_then(|val| val.parse::<u32>().ok()),
    };
    (new_name, new_value)
}

fn kodi_style_rename_season(name: &str, style: &KodiStyle, series_season: Option<&String>) -> (String, Option<u32>) {
    extract_season_or_episode_match_with_default(name, &style.season, series_season)
}

fn kodi_style_rename_episode(name: &str, style: &KodiStyle, series_episode: Option<&String>) -> (String, Option<u32>) {
    extract_season_or_episode_match_with_default(name, &style.episode, series_episode)
}

fn kodi_style_rename_year(
    name: &str,
    style: &KodiStyle,
    release_date: Option<&String>,
) -> (String, Option<u32>) {
    let (new_name, possible_year) = extract_match(name, &style.year);

    if let Some(year) = possible_year.as_deref() {
        if let Ok(num_year) = year.parse::<u32>() {
            let cur_year = u32::try_from(chrono::Utc::now().year()).unwrap_or(0);
            if (1900..=cur_year).contains(&num_year) {
                return (new_name, Some(num_year));
            }
        }
    }

    if let Some(rel_date) = release_date {
        if let Some(year) = extract_match(&rel_date, &style.year).1.and_then(|y| y.parse::<u32>().ok()) {
            return (name.to_string(), Some(year));
        }
    }
    (name.to_string(), None)
}

fn trim_string_after_pos(input: &str, start_pos: usize) -> Option<String> {
    if let Some(slice) = input.get(start_pos..) {
        if let Some(index) = slice.find(|c: char| c.is_alphanumeric()) {
            return Some(String::from(&slice[index..]));
        }
    }
    None
}

fn trim_whitespace(pattern: &Regex, input: &str) -> String {
    pattern.replace_all(input, " ").to_string()
}

async fn kodi_style_rename(cfg: &Config, strm_item_info: &StrmItemInfo, style: &KodiStyle, input_tmdb_indexes: &mut InputTmdbIndexMap, underscore_whitespace: bool) -> (PathBuf, String) {
    let separator = if underscore_whitespace { "_" } else { " " };
    let (name_1, year) = kodi_style_rename_year(&strm_item_info.title, style, strm_item_info.release_date.as_ref());
    let (name_2, season) = kodi_style_rename_season(&name_1, style, strm_item_info.season.as_ref());
    let (name_3, episode) = kodi_style_rename_episode(&name_2, style, strm_item_info.episode.as_ref());
    let name_4 = trim_whitespace(&style.whitespace, &*style.alphanumeric.replace_all(&name_3, ""));
    let title = &strm_item_info.series_name.as_ref()
        .filter(|&series_name| name_4.starts_with(series_name))
        .and_then(|series_name| trim_string_after_pos(&name_3, series_name.len()));
    let tmdb_value = match strm_item_info.item_type {
        PlaylistItemType::Series | PlaylistItemType::Video => get_tmdb_value(cfg, strm_item_info.provider_id, strm_item_info.input_id, input_tmdb_indexes, strm_item_info.item_type).await,
        _ => None,
    };

    let mut file_dir = vec![strm_item_info.group.to_string()];
    let mut filename = vec![];

    let name = if let Some(series_name) = &strm_item_info.series_name {
        series_name
    } else { &name_4 };
    let sanitized_name = sanitize_for_filename(name, underscore_whitespace);
    filename.push(sanitized_name.clone());

    if let Some(value) = year {
        filename.push(format!("{separator}({value})"));
        file_dir.push(format!("{sanitized_name}{separator}({value})"));
    } else {
        file_dir.push(sanitized_name);
    }

    if let Some(value) = season {
        filename.push(format!("{separator}S{value:02}"));
        file_dir.push(format!("Season{separator}{value}"));
    };

    if let Some(value) = episode {
        if season.is_none() {
            filename.push(separator.to_string());
        }
        filename.push(format!("E{value:02}"));
    };
    if let Some(value) = title {
        filename.push(format!("{separator}-{separator}{}", sanitize_for_filename(value, underscore_whitespace)));
    }


    if let Some(value) = tmdb_value {
        match value {
            InputTmdbIndexValue::Video(vod_record) => filename.push(format!("{separator}{{tmdb={}}}", vod_record.tmdb_id)),
            InputTmdbIndexValue::Series(episode) => filename.push(format!("{separator}{{tmdb={}}}", episode.tmdb_id)),
        }
    }
    let kodi_filename = filename.join("");

    let mut path = PathBuf::new();
    for dir in file_dir {
        path.push(sanitize_for_filename(&dir, underscore_whitespace));
    }
    (path, kodi_filename)
}

#[derive(Clone)]
enum InputTmdbIndexTree {
    Video(BPlusTree<u32, InputVodInfoRecord>),
    Series(BPlusTree<u32, XtreamSeriesEpisode>),
}

#[derive(Clone)]
enum InputTmdbIndexValue {
    Video(InputVodInfoRecord),
    Series(XtreamSeriesEpisode),
}

type InputTmdbIndexMap = HashMap<u16, Option<(FileReadGuard, InputTmdbIndexTree)>>;
async fn get_tmdb_value(cfg: &Config, provider_id: Option<u32>, input_id: u16,
                        input_indexes: &mut InputTmdbIndexMap, item_type: PlaylistItemType) -> Option<InputTmdbIndexValue> {
    // the tmdb_ids are stored inside record files for xtream input.
    // we load this record files on request for each input and item_type.
    match provider_id {
        None => None,
        Some(pid) => {
            match input_indexes.entry(input_id) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    if let Some((_, tree_value)) = entry.get() {
                        match tree_value {
                            InputTmdbIndexTree::Video(tree) => tree.query(&pid).map(|vod_record| InputTmdbIndexValue::Video(vod_record.clone())),
                            InputTmdbIndexTree::Series(tree) => tree.query(&pid).map(|episode| InputTmdbIndexValue::Series(episode.clone()))
                        }
                    } else {
                        None
                    }
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    if let Some(input) = cfg.get_input_by_id(input_id) {
                        if let Ok(Some(tmdb_path)) = get_input_storage_path(input, &cfg.working_dir)
                            .map(|storage_path| xtream_get_record_file_path(&storage_path, item_type)) {
                            if let Ok(file_lock) = cfg.file_locks.read_lock(&tmdb_path).await {
                                match item_type {
                                    PlaylistItemType::Series => {
                                        if let Ok(tree) = BPlusTree::<u32, XtreamSeriesEpisode>::load(&tmdb_path) {
                                            let tmdb_id = tree.query(&pid).map(|episode| InputTmdbIndexValue::Series(episode.clone()));
                                            entry.insert(Some((file_lock, InputTmdbIndexTree::Series(tree))));
                                            return tmdb_id;
                                        }
                                    }
                                    PlaylistItemType::Video => {
                                        if let Ok(tree) = BPlusTree::<u32, InputVodInfoRecord>::load(&tmdb_path) {
                                            let tmdb_id = tree.query(&pid).map(|vod_record| InputTmdbIndexValue::Video(vod_record.clone()));
                                            entry.insert(Some((file_lock, InputTmdbIndexTree::Video(tree))));
                                            return tmdb_id;
                                        }
                                    }
                                    _ => {}
                                }
                            };
                        };
                    }
                    entry.insert(None);
                    None
                }
            }
        }
    }
}

struct StrmItemInfo {
    group: Rc<String>,
    title: Rc<String>,
    item_type: PlaylistItemType,
    provider_id: Option<u32>,
    input_id: u16,
    url: Rc<String>,
    series_name: Option<String>,
    release_date: Option<String>,
    season: Option<String>,
    episode: Option<String>,
}

fn extract_item_info(pli: &PlaylistItem) -> StrmItemInfo {
    let mut header = pli.header.borrow_mut();
    let group = Rc::clone(&header.group);
    let title = Rc::clone(&header.title);
    let item_type = header.item_type;
    let provider_id = header.get_provider_id();
    let input_id = header.input_id;
    // TODO reverse proxy url
    let url = Rc::clone(&header.url);
    let (series_name, release_date, season, episode) = if header.item_type == PlaylistItemType::Series {
        let series_name = header.get_additional_property_as_str("series_name");
        let release_date = header.get_additional_property_as_str("release_date");
        let season = header.get_additional_property_as_str("season");
        let episode = header.get_additional_property_as_str("episode");
        (series_name, release_date, season, episode)
    } else { (None, None, None, None) };

    StrmItemInfo { group, title, item_type, provider_id, input_id, url, series_name, release_date, season, episode }
}

fn prepare_strm_output_directory(cleanup: bool, path: &PathBuf) -> Result<(), M3uFilterError> {
    if cleanup {
        let _ = std::fs::remove_dir_all(&path);
    }
    if let Err(e) = std::fs::create_dir_all(&path) {
        error!("cant create directory: {:?}", &path);
        return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write strm playlist: {}", e);
    };
    Ok(())
}

fn filter_strm_item(&pli: &&PlaylistItem) -> bool {
    let item_type = pli.header.borrow().item_type;
    item_type == PlaylistItemType::Series
        || item_type == PlaylistItemType::Live
        || item_type == PlaylistItemType::Video
}

fn get_strm_output_options(target: &ConfigTarget) -> (bool, bool, bool) {
    target.options.as_ref().map_or_else(
        || (false, false, false),
        |o| (o.underscore_whitespace, o.cleanup, o.kodi_style))
}

pub async fn kodi_write_strm_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &[PlaylistGroup], filename: Option<&str>) -> Result<(), M3uFilterError> {
    let mut result = Ok(());
    if !new_playlist.is_empty() {
        if filename.is_none() {
            return Err(notify_err!("write strm playlist failed: ".to_string()));
        }
        let (underscore_whitespace, cleanup, kodi_style) = get_strm_output_options(target);
        let Some(path) = file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&filename.as_ref().unwrap()))) else {
            return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Failed to get file path for {}", filename.unwrap_or(""));
        };
        let _ = prepare_strm_output_directory(cleanup, &path)?;
        let mut input_tmdb_indexes: InputTmdbIndexMap = HashMap::new();
        for pg in new_playlist {
            for pli in pg.channels.iter().filter(filter_strm_item) {
                // we need to consider
                // - Live streams
                // - Xtream Series Episode (has series_name and release_date)
                // - Xtream VOD (should have year or release_date)
                // - M3u Series (TODO we dont have this currently, should be guessed through m3u parser)
                // - M3u Vod (no additional infos, need to extract from title)

                let str_item_info = extract_item_info(pli);
                let (dir_path, strm_file_name) = if kodi_style {
                    kodi_style_rename(cfg, &str_item_info, &KODI_STYLE, &mut input_tmdb_indexes, underscore_whitespace).await
                } else {
                    let dir_path = path.join(sanitize_for_filename(&str_item_info.group, underscore_whitespace));
                    let strm_file_name = sanitize_for_filename(&str_item_info.title, underscore_whitespace);
                    (dir_path, strm_file_name)
                };
                let output_path = path.join(dir_path);
                if let Err(e) = std::fs::create_dir_all(&output_path) {
                    error!("cant create directory: {output_path:?}");
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to create directory for strm playlist:{output_path:?} {e}");
                };
                let file_path = output_path.join(format!("{strm_file_name}.strm"));
                match File::create(&file_path) {
                    Ok(mut strm_file) => {
                        match file_utils::check_write(&strm_file.write_all(str_item_info.url.as_bytes())) {
                            Ok(()) => {}
                            Err(err) => {
                                error!("failed to write strm playlist: {err}");
                                result = Err(notify_err!(format!("failed to write strm playlist: {}", err)));
                            }
                        }
                    }
                    Err(err) => {
                        error!("failed to write strm playlist: {err}");
                        result = Err(notify_err!(format!("failed to write strm playlist: {}", err)));
                    }
                };
            }
        }
    }
    result
}
