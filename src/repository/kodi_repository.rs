use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget};
use crate::model::playlist::{PlaylistGroup, PlaylistItemType};
use crate::model::xtream::XtreamSeriesInfoEpisode;
use crate::repository::bplustree::BPlusTree;
use crate::repository::storage::get_input_storage_path;
use crate::repository::xtream_repository::xtream_get_record_file_path;
use crate::utils::file_lock_manager::FileReadGuard;
use crate::utils::file_utils;
use crate::{create_m3u_filter_error_result, notify_err};
use chrono::Datelike;
use log::error;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use std::sync::LazyLock;

struct KodiStyle {
    year: regex::Regex,
    season: regex::Regex,
    episode: regex::Regex,
    whitespace: regex::Regex,
}

fn sanitize_for_filename(text: &str, underscore_whitespace: bool) -> String {
    text.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .map(|c| if underscore_whitespace { if c.is_whitespace() { '_' } else { c } } else { c })
        .collect::<String>()
}

fn kodi_style_rename_year(name: &str, style: &KodiStyle) -> (String, Option<String>) {
    let current_date = chrono::Utc::now();
    let cur_year = current_date.year();
    match style.year.find(name) {
        Some(m) => {
            let s_year = &name[m.start()..m.end()];
            let t_year: i32 = s_year.parse().unwrap();
            if t_year > 1900 && t_year <= cur_year {
                let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
                return (new_name, Some(String::from(s_year)));
            }
            (String::from(name), Some(cur_year.to_string()))
        }
        _ => (String::from(name), Some(cur_year.to_string())),
    }
}

fn kodi_style_rename_season(name: &str, style: &KodiStyle) -> (String, Option<String>) {
    style.season.find(name).map_or_else(|| (String::from(name), Some(String::from("01"))), |m| {
        let s_season = &name[m.start()..m.end()];
        let season = Some(String::from(&s_season[1..]));
        let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
        (new_name, season)
    })
}

fn kodi_style_rename_episode(name: &str, style: &KodiStyle) -> (String, Option<String>) {
    style.episode.find(name).map_or_else(|| (String::from(name), None), |m| {
        let s_episode = &name[m.start()..m.end()];
        let episode = Some(String::from(&s_episode[1..]));
        let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
        (new_name, episode)
    })
}

fn kodi_style_rename(name: &str, style: &KodiStyle) -> (Vec<String>, String) {
    let (work_name, year) = kodi_style_rename_year(name, style);

    // TODO use this for non xtream input
    // let (work_name_2, season) = kodi_style_rename_season(&work_name_1, style);
    // let (work_name_3, episode) = kodi_style_rename_episode(&work_name_2, style);

    let mut filename = work_name;
    let mut filedir = vec![String::from(style.whitespace.replace_all(filename.as_str(), " "))];
    if year.is_some() {
        if year.is_some() {
            filename = format!("{filename} ({})", year.unwrap());
            filedir = vec![String::from(style.whitespace.replace_all(filename.as_str(), " "))];
        }
        return (filedir, String::from(style.whitespace.replace_all(filename.as_str(), " ")));
    }
    let new_name = String::from(style.whitespace.replace_all(name, " "));
    (vec![new_name.clone()], new_name)
}

static KODI_STYLE: LazyLock<KodiStyle> = LazyLock::new(|| KodiStyle {
    season: regex::Regex::new(r"[Ss]\d\d").unwrap(),
    episode: regex::Regex::new(r"[Ee]\d\d").unwrap(),
    year: regex::Regex::new(r"\d\d\d\d").unwrap(),
    whitespace: regex::Regex::new(r"\s+").unwrap(),
});

#[derive(Clone)]
enum InputTmdbIndexTree {
    Video(BPlusTree<u32, u32>),
    Series(BPlusTree<u32, XtreamSeriesInfoEpisode>),
}

#[derive(Clone)]
enum InputTmdbIndexValue {
    Video(u32),
    Series(XtreamSeriesInfoEpisode),
}

type InputTmdbIndexMap = HashMap<u16, Option<(FileReadGuard, InputTmdbIndexTree)>>;
async fn get_tmdb_value(cfg: &Config, provider_id: Option<u32>, input_id: u16,
                        input_indexes: &mut InputTmdbIndexMap, item_type: PlaylistItemType) -> Option<InputTmdbIndexValue> {
    match provider_id {
        None => None,
        Some(pid) => {
            match input_indexes.entry(input_id) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    if let Some((_, tree_value)) = entry.get() {
                        match tree_value {
                            InputTmdbIndexTree::Video(tree) => tree.query(&pid).map(|id| InputTmdbIndexValue::Video(*id)),
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
                                        if let Ok(tree) = BPlusTree::<u32, XtreamSeriesInfoEpisode>::load(&tmdb_path) {
                                            let tmdb_id = tree.query(&pid).map(|episode| InputTmdbIndexValue::Series(episode.clone()));
                                            entry.insert(Some((file_lock, InputTmdbIndexTree::Series(tree))));
                                            return tmdb_id;
                                        }
                                    }
                                    PlaylistItemType::Video => {
                                        if let Ok(tree) = BPlusTree::<u32, u32>::load(&tmdb_path) {
                                            let tmdb_id = tree.query(&pid).map(|id| InputTmdbIndexValue::Video(*id));
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

pub async fn kodi_write_strm_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &[PlaylistGroup], filename: Option<&str>) -> Result<(), M3uFilterError> {
    if !new_playlist.is_empty() {
        if filename.is_none() {
            return Err(notify_err!("write strm playlist failed: ".to_string()));
        }
        let underscore_whitespace = target.options.as_ref().is_some_and(|o| o.underscore_whitespace);
        let cleanup = target.options.as_ref().is_some_and(|o| o.cleanup);
        let kodi_style = target.options.as_ref().is_some_and(|o| o.kodi_style);

        if let Some(path) = file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&filename.as_ref().unwrap()))) {
            if cleanup {
                let _ = std::fs::remove_dir_all(&path);
            }
            if let Err(e) = std::fs::create_dir_all(&path) {
                error!("cant create directory: {:?}", &path);
                return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write strm playlist: {}", e);
            };

            let mut input_tmdb_indexes: InputTmdbIndexMap = HashMap::new();

            for pg in new_playlist {
                for pli in pg.channels.iter().filter(|&pli| {
                    let item_type = pli.header.borrow().item_type;
                    item_type == PlaylistItemType::Series
                        || item_type == PlaylistItemType::Live
                        || item_type == PlaylistItemType::Video
                }) {
                    let (group, title, item_type, provider_id, input_id, url) = {
                        let mut header = pli.header.borrow_mut();
                        let group = Rc::clone(&header.group);
                        let title = Rc::clone(&header.title);
                        let item_type = header.item_type;
                        let provider_id = header.get_provider_id();
                        let input_id = header.input_id;
                        let url = Rc::clone(&header.url);
                        (group, title, item_type, provider_id, input_id, url)
                    };

                    let mut dir_path = path.join(sanitize_for_filename(&group, underscore_whitespace));
                    let mut kodi_file_name = sanitize_for_filename(&title, underscore_whitespace);
                    let mut additional_info = String::new();
                    if kodi_style {
                        let (kodi_file_dir_name, kodi_style_filename) = kodi_style_rename(&kodi_file_name, &KODI_STYLE);
                        kodi_file_name = kodi_style_filename;
                        kodi_file_dir_name.iter().for_each(|p| dir_path = dir_path.join(p));

                        let tmdb_value = match item_type {
                            PlaylistItemType::Series | PlaylistItemType::Video => get_tmdb_value(cfg, provider_id, input_id, &mut input_tmdb_indexes, item_type).await,
                            _ => None,
                        };
                        additional_info = match tmdb_value {
                            None => { String::new() }
                            Some(value) => {
                                match value {
                                    InputTmdbIndexValue::Video(tmdb_id) => format!(" {{tmdb={tmdb_id}}}"),
                                    InputTmdbIndexValue::Series(episode) => {
                                        let mut episode_ext = format!(" S{:02}E{:02}", episode.season, episode.episode_num);
                                        if let Some(tmdb_id) = episode.info.tmdb_id {
                                            episode_ext = format!("{episode_ext} {{tmdb={tmdb_id}}}");
                                        }
                                        episode_ext
                                    }
                                }
                            }
                        };
                    }
                    if let Err(e) = std::fs::create_dir_all(&dir_path) {
                        error!("cant create directory: {:?}", &dir_path);
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write strm playlist: {}", e);
                    };
                    let file_path = dir_path.join(format!("{kodi_file_name}{additional_info}.strm"));
                    match File::create(&file_path) {
                        Ok(mut strm_file) => {
                            match file_utils::check_write(&strm_file.write_all(url.as_bytes())) {
                                Ok(()) => (),
                                Err(e) => return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write strm playlist: {}", e),
                            }
                        }
                        Err(err) => {
                            return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write strm playlist: {}", err);
                        }
                    }
                }
            }
        }
    }
    Ok(())
}