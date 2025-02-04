use std::path::{Path, PathBuf};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::{ApiProxyServerInfo, ProxyType, ProxyUserCredentials};
use crate::model::config::{Config, ConfigTarget, TargetOutput};
use crate::model::playlist::{FieldGetAccessor, PlaylistGroup, PlaylistItem, PlaylistItemType, UUIDType};
use crate::model::xtream::XtreamSeriesEpisode;
use crate::repository::bplustree::BPlusTree;
use crate::repository::storage::{ensure_target_storage_path, get_input_storage_path, hash_bytes, FILE_SUFFIX_DB};
use crate::repository::xtream_repository::{xtream_get_record_file_path, InputVodInfoRecord};
use crate::utils::file_lock_manager::FileReadGuard;
use crate::utils::file_utils;
use crate::utils::request_utils::extract_extension_from_url;
use crate::{create_m3u_filter_error_result, info_err, notify_err};
use async_std::io::{BufReadExt, BufWriter, BufReader, ReadExt, WriteExt};
use async_std::fs::{File, read_dir, remove_dir, remove_file, create_dir_all};
use chrono::Datelike;
use futures::{StreamExt};
use log::error;
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::LazyLock;
use filetime::{set_file_times, FileTime};

const FILE_STRM: &str = "strm";

struct KodiStyle {
    year: Regex,
    season: Regex,
    episode: Regex,
    whitespace: Regex,
    alphanumeric: Regex,
}

static KODI_STYLE: LazyLock<KodiStyle> = LazyLock::new(|| KodiStyle {
    season: Regex::new(r"[Ss]\d{1,2}").unwrap(),
    episode: Regex::new(r"[Ee]\d{1,2}").unwrap(),
    year: Regex::new(r"\d{4}").unwrap(),
    whitespace: Regex::new(r"\s+").unwrap(),
    alphanumeric: Regex::new(r"[^\w\s]").unwrap(),
});

fn sanitize_for_filename(text: &str, underscore_whitespace: bool) -> String {
    text.trim().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace() || (*c == '(') || (*c == ')'))
        .map(|c| if underscore_whitespace { if c.is_whitespace() { '_' } else { c } } else { c })
        .collect::<String>()
}

fn extract_match(name: &str, pattern: &Regex) -> (String, Option<String>) {
    pattern.find(name).map_or_else(|| (name.to_string(), None), |m| {
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

fn kodi_style_rename_year<'a>(
    name: &'a str,
    style: &KodiStyle,
    release_date: Option<&'a String>,
) -> (&'a str, Option<u32>) {
    let mut years = Vec::new();

    let cur_year = u32::try_from(chrono::Utc::now().year()).unwrap_or(0);
    let mut new_name = String::with_capacity(name.len());
    let mut last_index = 0;

    for caps in style.year.captures_iter(name) {
        if let Ok(year) = caps[0].parse::<u32>() {
            if (1900..=cur_year).contains(&year) {
                years.push(year);
                let match_start = caps.get(0).unwrap().start();
                let match_end = caps.get(0).unwrap().end();
                new_name.push_str(&name[last_index..match_start]);
                last_index = match_end;
            }
        }
    }
    new_name.push_str(&name[last_index..]);

    let smallest_year = years.into_iter().min();
    if smallest_year.is_none() {
        if let Some(rel_date) = release_date {
            if let Some(year) = extract_match(rel_date, &style.year)
                .1
                .and_then(|y| y.parse::<u32>().ok())
            {
                return (name, Some(year));
            }
        }
    }

    (Box::leak(new_name.into_boxed_str()), smallest_year)
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
    let (name_1, year_from_title) = kodi_style_rename_year(&strm_item_info.title, style, strm_item_info.release_date.as_ref());
    let mut series_year = year_from_title;
    let (name_2, season) = kodi_style_rename_season(name_1, style, strm_item_info.season.as_ref());
    let (name_3, episode) = kodi_style_rename_episode(&name_2, style, strm_item_info.episode.as_ref());
    let name_4 = trim_whitespace(&style.whitespace, &style.alphanumeric.replace_all(&name_3, ""));
    let title = &strm_item_info.series_name.as_ref()
        .filter(|&series_name| name_4.starts_with(series_name))
        .and_then(|series_name| trim_string_after_pos(&name_3, series_name.len()));
    let tmdb_id = if let Some(value) = match strm_item_info.item_type {
        PlaylistItemType::Series | PlaylistItemType::Video => get_tmdb_value(cfg, strm_item_info.provider_id, strm_item_info.input_name.as_str(), input_tmdb_indexes, strm_item_info.item_type).await,
        _ => None,
    } {
        match value {
            InputTmdbIndexValue::Video(vod_record) => vod_record.tmdb_id,
            InputTmdbIndexValue::Series(episode) => episode.tmdb_id,
        }
    } else { 0 };

    let mut file_dir = vec![strm_item_info.group.to_string()];
    let mut filename = vec![];

    let sanitized_name = sanitize_for_filename(&name_4, underscore_whitespace);
    filename.push(sanitized_name.clone());

    let dir_name = if let Some(series_name) = &strm_item_info.series_name {
        let (folder_name, year) = kodi_style_rename_year(series_name, style, strm_item_info.release_date.as_ref());
        if let (Some(y), Some(sy)) = (year, series_year) {
            if y < sy {
                series_year = year;
            }
        }
        trim_whitespace(&style.whitespace, &style.alphanumeric.replace_all(folder_name, ""))
    } else { name_4 };

    let sanitized_dir_name = sanitize_for_filename(&dir_name, underscore_whitespace);
    if let Some(value) = series_year {
        filename.push(format!("{separator}({value})"));
        file_dir.push(format!("{sanitized_dir_name}{separator}({value})"));
    } else {
        file_dir.push(sanitized_dir_name);
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
        let sanitized_value = sanitize_for_filename(&trim_whitespace(&style.whitespace, &style.alphanumeric.replace_all(value, "")), underscore_whitespace);
        if !filename.iter().any(|e| e.contains(&sanitized_value)) {
            filename.push(format!("{separator}-{separator}{sanitized_value}", ));
        }
    }

    if tmdb_id > 0 {
        filename.push(format!("{separator}{{tmdb={tmdb_id}}}"));
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

type InputTmdbIndexMap = HashMap<String, Option<(FileReadGuard, InputTmdbIndexTree)>>;
async fn get_tmdb_value(cfg: &Config, provider_id: Option<u32>, input_name: &str,
                        input_indexes: &mut InputTmdbIndexMap, item_type: PlaylistItemType) -> Option<InputTmdbIndexValue> {
    // the tmdb_ids are stored inside record files for xtream input.
    // we load this record files on request for each input and item_type.
    match provider_id {
        None => None,
        Some(pid) => {
            match input_indexes.entry(input_name.to_string()) {
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
                    if let Some(input) = cfg.get_input_by_name(input_name) {
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

pub fn strm_get_file_paths(target_path: &Path) -> PathBuf {
    target_path.join(PathBuf::from(format!("{FILE_STRM}.{FILE_SUFFIX_DB}")))
}


#[derive(Serialize)]
struct StrmItemInfo {
    group: Rc<String>,
    title: Rc<String>,
    item_type: PlaylistItemType,
    provider_id: Option<u32>,
    virtual_id: u32,
    input_name: Rc<String>,
    url: Rc<String>,
    series_name: Option<String>,
    release_date: Option<String>,
    season: Option<String>,
    episode: Option<String>,
    added: Option<u64>,
}

impl StrmItemInfo {
    pub(crate) fn get_file_ts(&self) -> Option<u64> {
        self.added
    }
}

fn extract_item_info(pli: &PlaylistItem) -> StrmItemInfo {
    let mut header = pli.header.borrow_mut();
    let group = Rc::clone(&header.group);
    let title = Rc::clone(&header.title);
    let item_type = header.item_type;
    let provider_id = header.get_provider_id();
    let virtual_id = header.virtual_id;
    let input_name = Rc::clone(&header.input_name);
    let url = Rc::clone(&header.url);
    let (series_name, release_date, added, season, episode) = match header.item_type {
        PlaylistItemType::Series => {
            let series_name = match header.get_field("name") {
                Some(name) if !name.is_empty() => Some(name.to_string()),
                _ => header.get_additional_property_as_str("series_name"),
            };
            let release_date = header.get_additional_property_as_str("series_release_date")
                .or_else(|| header.get_additional_property_as_str("release_date"));
            let season = header.get_additional_property_as_str("season");
            let episode = header.get_additional_property_as_str("episode");
            let added = header.get_additional_property_as_u64("added");
            (series_name, release_date, added,  season, episode)
        },
        PlaylistItemType::Video => {
            let name = header.get_field("name").map(|v| v.to_string());
            let release_date = header.get_additional_property_as_str("release_date");
            let added = header.get_additional_property_as_u64("added");
            (name, release_date, added, None, None)
        },
        _ => { (None, None, None, None, None) }
    };
    StrmItemInfo { group, title, item_type, provider_id, virtual_id, input_name, url, series_name, release_date, season, episode, added }
}

async fn prepare_strm_output_directory(path: &Path) -> Result<(), M3uFilterError> {
    // Ensure the directory exists
    if let Err(e) = async_std::fs::create_dir_all(path).await {
        error!("Failed to create directory {path:?}: {e}");
        return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Error creating STRM directory: {e}");
    }
    Ok(())
}
async fn cleanup_strm_output_directory(
    cleanup: bool,
    root_path: &Path,
    existing: &HashSet<String>,
    processed: &HashSet<String>,
) -> Result<(), String> {
    if !(root_path.exists() && root_path.is_dir()) {
        return Err(format!("Error: STRM directory does not exist: {root_path:?}"));
    }

    let to_remove: HashSet<String> = if cleanup {
        // Remove al files which are not in `processed`
        let mut found_files = HashSet::new();

        let mut entries = read_dir(root_path).await.map_err(|e| format!("Failed to read directory {root_path:?}: {e}"))?;
        while let Some(entry) = entries.next().await {
            let entry = entry.map_err(|e| format!("Error retrieving directory entry: {e}"))?;
            if entry.file_type().await.map_err(|e| format!("Failed to get file type for {entry:?}: {e}"))?.is_file() {
                if let Some(file_name) = entry.path().strip_prefix(root_path).ok().and_then(|p| p.to_str()) {
                    found_files.insert(file_name.to_string());
                }
            }
        }
        &found_files - processed
    } else {
        // Remove all files from `existing`, which are not in `processed`
        existing - processed
    };

    for file in &to_remove {
        let file_path = root_path.join(file);
        if let Err(err) = remove_file(&file_path).await {
            eprintln!("Failed to remove file {file_path:?}: {err}");
        }
    }

    // TODO should we delete all empty directories if cleanup=false ?
    remove_empty_dirs(root_path.into()).await?;

    Ok(())
}

async fn remove_empty_dirs(root_path: async_std::path::PathBuf) -> Result<(), String> {
    let mut stack = vec![root_path];
    let mut dirs_to_delete = Vec::new();
    let mut ignore_root = true;

    while let Some(dir) = stack.pop() {
        let mut entries = match read_dir(&dir).await {
            Ok(entries) => entries,
            Err(err) => {
                error!("Error reading directory {dir:?}: {err}");
                continue;
            }
        };

        let mut has_files = false;

        while let Some(entry) = entries.next().await {
            match entry {
                Ok(entry) => {
                    if entry.file_type().await.map_err(|e| format!("Failed to get file type for {entry:?}: {e}"))?.is_dir() {
                        stack.push(entry.path());
                    } else {
                        has_files = true;
                    }
                }
                Err(err) => {
                    error!("Error retrieving directory entry: {dir:?} {err}");
                }
            }
        }

        if !ignore_root && !has_files {
            dirs_to_delete.push(dir);
        }
        ignore_root = false;
    }

    // Delete directories from bottom to top
    for dir in dirs_to_delete.into_iter().rev() {
        if let Err(e) = remove_dir(&dir).await {
            eprintln!("Failed to remove empty directory {dir:?}: {e}");
        }
    }

    Ok(())
}


fn filter_strm_item(pli: &PlaylistItem) -> bool {
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

fn get_relative_path_str(full_path: &Path, root_path: &Path) -> String {
    full_path.strip_prefix(root_path).map_or_else(|_| full_path.to_string_lossy(), |relative| relative.to_string_lossy())
        .to_string()
}

pub async fn kodi_write_strm_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &[PlaylistGroup], output: &TargetOutput) -> Result<(), M3uFilterError> {
    if new_playlist.is_empty() {
        return Ok(());
    }
    if output.filename.is_none() {
        return Err(notify_err!("write strm playlist failed: ".to_string()));
    }

    let Some(root_path) = file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&output.filename.as_ref().unwrap()))) else {
        return Err(info_err!(format!("Failed to get file path for {}", output.filename.as_deref().unwrap_or(""))));
    };

    let credentials_and_server_info = get_credentials_and_server_info(cfg, output);
    let (underscore_whitespace, cleanup, kodi_style) = get_strm_output_options(target);
    let strm_index_path = strm_get_file_paths(&ensure_target_storage_path(cfg, target.name.as_str())?);
    let existing_strm = {
        let _file_lock = cfg.file_locks.read_lock(&strm_index_path).await.map_err(|err| info_err!(format!("{err}")))?;
        read_strm_file_index(&strm_index_path).await.unwrap_or_else(|_| HashSet::with_capacity(4096))
    };
    let mut processed_strm: HashSet<String> = HashSet::with_capacity(existing_strm.len());

    let mut failed = vec![];
    let strm_props = target.options.as_ref().and_then(|o| o.strm_props.as_ref());

    prepare_strm_output_directory(&root_path).await?;
    let mut input_tmdb_indexes: InputTmdbIndexMap = HashMap::new();

    for pg in new_playlist {
        for pli in pg.channels.iter().filter(|&pli| filter_strm_item(pli)) {
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
                let dir_path = root_path.join(sanitize_for_filename(&str_item_info.group, underscore_whitespace));
                let strm_file_name = sanitize_for_filename(&str_item_info.title, underscore_whitespace);
                (dir_path, strm_file_name)
            };

            // file paths
            let output_path = root_path.join(dir_path);
            let file_path = output_path.join(format!("{strm_file_name}.strm"));
            let file_exists = file_path.exists();
            let relative_file_path = get_relative_path_str(&file_path, &root_path);

            // create content
            let url = get_strm_url(credentials_and_server_info.as_ref(), &str_item_info);
            let mut content = strm_props.map_or_else(Vec::new, std::clone::Clone::clone);
            content.push(url);
            let content_text = content.join("\r\n");
            let content_as_bytes = content_text.as_bytes();
            let content_hash = hash_bytes(content_as_bytes);

            // check if file exists and has same hash
            if file_exists && has_strm_file_same_hash(&file_path, content_hash).await {
                processed_strm.insert(relative_file_path);
                continue; // skip creation
            }

            // if we cant create the directory skip this entry
            if !ensure_strm_file_directory(&mut failed, &output_path).await { continue; }

            match write_strm_file(&file_path, content_as_bytes, str_item_info.get_file_ts()).await {
                Ok(()) => { processed_strm.insert(relative_file_path); }
                Err(err) => { failed.push(err); }
            };
        }
    }

    if let Err(err) = write_strm_index_file(cfg, &processed_strm, &strm_index_path).await {
        failed.push(err);
     };

    if let Err(err) = cleanup_strm_output_directory(cleanup, &root_path, &existing_strm, &processed_strm).await {
        failed.push(err);
    }

    if failed.is_empty() {
        Ok(())
    } else {
        Err(info_err!(failed.join(", ")))
    }
}

async fn write_strm_index_file(cfg: &Config, entries: &HashSet<String>, index_file_path: &PathBuf) -> Result<(), String> {
    let _file_lock = cfg.file_locks.write_lock(index_file_path).await.map_err(|err| format!("{err}"))?;
    let file = File::create(index_file_path).await.map_err(|err| format!("Failed to create strm index file: {index_file_path:?} {err}"))?;
    let mut writer = BufWriter::new(file);
    let new_line = "\n".as_bytes();
    for entry in entries {
        writer.write_all(entry.as_bytes()).await.map_err(|err| format!("failed to write strm index entry: {err}"))?;
        writer.write(new_line).await.map_err(|err| format!("failed to write strm index entry: {err}"))?;
    }
    writer.flush().await.map_err(|err| format!("failed to write strm index entry: {err}"))?;
    Ok(())
}

async fn ensure_strm_file_directory(failed: &mut Vec<String>, output_path: &Path) -> bool {
    if !output_path.exists() {
        if let Err(e) = create_dir_all(output_path).await {
            let err_msg = format!("Failed to create directory for strm playlist: {output_path:?} {e}");
            error!("{}", err_msg);
            failed.push(err_msg);
            return false; // skip creation, could not create directory
        };
    }
    true
}

async fn write_strm_file(file_path: &Path, content_as_bytes: &[u8], timestamp: Option<u64>) -> Result<(), String> {
    File::create(file_path).await
        .map_err(|err| format!("failed to create strm file: {err}"))?
        .write_all(content_as_bytes).await
        .map_err(|err| format!("failed to write strm playlist: {err}"))?;

    if let Some(ts) = timestamp {
        #[allow(clippy::cast_possible_wrap)]
        let mtime = FileTime::from_unix_time(ts as i64, 0); // Unix-Timestamp: 01.01.2023 00:00:00 UTC
        #[allow(clippy::cast_possible_wrap)]
        let atime = FileTime::from_unix_time(ts as i64, 0); // access time
        let _ = set_file_times(file_path, mtime, atime);
    }

    Ok(())
}

async fn has_strm_file_same_hash(file_path: &PathBuf, content_hash: UUIDType) -> bool {
    if let Ok(file) = File::open(&file_path).await {
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        if reader.read_to_end(&mut buffer).await.is_ok() {
            let file_hash = hash_bytes(&buffer);
            if content_hash == file_hash {
                return true;
            }
        };
    }
    false
}

fn get_credentials_and_server_info(cfg: &Config, output: &TargetOutput) -> Option<(ProxyUserCredentials, ApiProxyServerInfo)> {
    output.username.as_ref()
        .and_then(|username| cfg.get_user_credentials(username))
        .filter(|credentials| credentials.proxy == ProxyType::Reverse)
        .map(|credentials| {
            let server_info = cfg.get_user_server_info(&credentials);
            (credentials, server_info)
        })
}

async fn read_strm_file_index(strm_file_index_path: &Path) -> std::io::Result<HashSet<String>> {
    let file = File::open(strm_file_index_path).await?;
    let reader = BufReader::new(file);
    let mut result = HashSet::new();
    let mut lines = reader.lines();
    while let Some(Ok(line)) = lines.next().await {
        result.insert(line);
    }
    Ok(result)
}

fn get_strm_url(credentials_and_server_info: Option<&(ProxyUserCredentials, ApiProxyServerInfo)>, str_item_info: &StrmItemInfo) -> String {
    credentials_and_server_info.as_ref()
        .map_or_else(|| str_item_info.url.to_string(),
                     |(user, server_info)|
                         if let Some(stream_type) = match str_item_info.item_type {
                             PlaylistItemType::Series => Some("series"),
                             PlaylistItemType::Live => Some("live"),
                             PlaylistItemType::Video => Some("movie"),
                             _ => None,
                         } {
                             let url = str_item_info.url.as_str();
                             let ext = extract_extension_from_url(url).map_or_else(String::new, std::string::ToString::to_string);
                             format!("{}/{stream_type}/{}/{}/{}{ext}",
                                     server_info.get_base_url(),
                                     user.username,
                                     user.password,
                                     str_item_info.virtual_id)
                         } else {
                             str_item_info.url.to_string()
                         },
        )
}
