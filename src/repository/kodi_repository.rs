use crate::m3u_filter_error::{create_m3u_filter_error_result, info_err};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::{ApiProxyServerInfo, ProxyType, ProxyUserCredentials};
use crate::model::config::{Config, ConfigTarget, StrmTargetOutput};
use crate::model::playlist::{
    FieldGetAccessor, PlaylistGroup, PlaylistItem, PlaylistItemType, UUIDType,
};
use crate::model::xtream::XtreamSeriesEpisode;
use crate::repository::bplustree::BPlusTree;
use crate::repository::storage::{
    ensure_target_storage_path, get_input_storage_path, hash_bytes, FILE_SUFFIX_DB,
};
use crate::repository::xtream_repository::{xtream_get_record_file_path, InputVodInfoRecord};
use crate::utils::file::file_lock_manager::FileReadGuard;
use crate::utils::file::file_utils;
use crate::utils::network::request::extract_extension_from_url;
use chrono::Datelike;
use filetime::{set_file_times, FileTime};
use log::{error, trace};
use regex::Regex;
use serde::Serialize;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use tokio::fs::{create_dir_all, remove_dir, remove_file, File};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

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
    text.trim()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || (*c == '(') || (*c == ')'))
        .map(|c| {
            if underscore_whitespace {
                if c.is_whitespace() {
                    '_'
                } else {
                    c
                }
            } else {
                c
            }
        })
        .collect::<String>()
}

fn extract_match(name: &str, pattern: &Regex) -> (String, Option<String>) {
    pattern.find(name).map_or_else(
        || (name.to_string(), None),
        |m| {
            let matched = String::from(&name[m.start()..m.end()]);
            let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
            (new_name, Some(matched))
        },
    )
}

fn extract_season_or_episode_match_with_default(
    name: &str,
    pattern: &Regex,
    default_value: Option<&String>,
) -> (String, Option<u32>) {
    let (new_name, value) = extract_match(name, pattern);
    let new_value = match value {
        // we skip the prefix S or E to parse the num
        Some(num_value) => num_value[1..].parse::<u32>().ok(),
        None => default_value.and_then(|val| val.parse::<u32>().ok()),
    };
    (new_name, new_value)
}

fn kodi_style_rename_season(
    name: &str,
    style: &KodiStyle,
    series_season: Option<&String>,
) -> (String, Option<u32>) {
    extract_season_or_episode_match_with_default(name, &style.season, series_season)
}

fn kodi_style_rename_episode(
    name: &str,
    style: &KodiStyle,
    series_episode: Option<&String>,
) -> (String, Option<u32>) {
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

async fn kodi_style_rename(
    cfg: &Config,
    strm_item_info: &StrmItemInfo,
    style: &KodiStyle,
    input_tmdb_indexes: &mut InputTmdbIndexMap,
    underscore_whitespace: bool,
) -> (PathBuf, String) {
    let separator = if underscore_whitespace { "_" } else { " " };
    let (name_1, year_from_title) = kodi_style_rename_year(
        &strm_item_info.title,
        style,
        strm_item_info.release_date.as_ref(),
    );
    let mut series_year = year_from_title;
    let (name_2, season) = kodi_style_rename_season(name_1, style, strm_item_info.season.as_ref());
    let (name_3, episode) =
        kodi_style_rename_episode(&name_2, style, strm_item_info.episode.as_ref());
    let name_4 = trim_whitespace(
        &style.whitespace,
        &style.alphanumeric.replace_all(&name_3, ""),
    );
    let title = &strm_item_info
        .series_name
        .as_ref()
        .filter(|&series_name| name_4.starts_with(series_name))
        .and_then(|series_name| trim_string_after_pos(&name_3, series_name.len()));
    let tmdb_id = if let Some(value) = match strm_item_info.item_type {
        PlaylistItemType::Series | PlaylistItemType::Video => {
            get_tmdb_value(
                cfg,
                strm_item_info.provider_id,
                strm_item_info.input_name.as_str(),
                input_tmdb_indexes,
                strm_item_info.item_type,
            ).await
        }
        _ => None,
    } {
        match value {
            InputTmdbIndexValue::Video(vod_record) => vod_record.tmdb_id,
            InputTmdbIndexValue::Series(episode) => episode.tmdb_id,
        }
    } else {
        0
    };

    let mut file_dir = vec![strm_item_info.group.to_string()];
    let mut filename = vec![];

    let sanitized_name = sanitize_for_filename(&name_4, underscore_whitespace);
    filename.push(sanitized_name.clone());

    let dir_name = if let Some(series_name) = &strm_item_info.series_name {
        let (folder_name, year) =
            kodi_style_rename_year(series_name, style, strm_item_info.release_date.as_ref());
        if let (Some(y), Some(sy)) = (year, series_year) {
            if y < sy {
                series_year = year;
            }
        }
        trim_whitespace(
            &style.whitespace,
            &style.alphanumeric.replace_all(folder_name, ""),
        )
    } else {
        name_4
    };

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
    }

    if let Some(value) = episode {
        if season.is_none() {
            filename.push(separator.to_string());
        }
        filename.push(format!("E{value:02}"));
    }
    if let Some(value) = title {
        let sanitized_value = sanitize_for_filename(
            &trim_whitespace(
                &style.whitespace,
                &style.alphanumeric.replace_all(value, ""),
            ),
            underscore_whitespace,
        );
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
async fn get_tmdb_value(
    cfg: &Config,
    provider_id: Option<u32>,
    input_name: &str,
    input_indexes: &mut InputTmdbIndexMap,
    item_type: PlaylistItemType,
) -> Option<InputTmdbIndexValue> {
    // the tmdb_ids are stored inside record files for xtream input.
    // we load this record files on request for each input and item_type.
    let pid = provider_id?;
    match input_indexes.entry(input_name.to_string()) {
        std::collections::hash_map::Entry::Occupied(entry) => {
            if let Some((_, tree_value)) = entry.get() {
                match tree_value {
                    InputTmdbIndexTree::Video(tree) => tree
                        .query(&pid)
                        .map(|vod_record| InputTmdbIndexValue::Video(vod_record.clone())),
                    InputTmdbIndexTree::Series(tree) => tree
                        .query(&pid)
                        .map(|episode| InputTmdbIndexValue::Series(episode.clone())),
                }
            } else {
                None
            }
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            if let Ok(Some(tmdb_path)) = get_input_storage_path(input_name, &cfg.working_dir)
                .map(|storage_path| xtream_get_record_file_path(&storage_path, item_type))
            {
                {
                    let file_lock = cfg.file_locks.read_lock(&tmdb_path).await;
                    match item_type {
                        PlaylistItemType::Series => {
                            if let Ok(tree) =
                                BPlusTree::<u32, XtreamSeriesEpisode>::load(&tmdb_path)
                            {
                                let tmdb_id = tree.query(&pid).map(|episode| {
                                    InputTmdbIndexValue::Series(episode.clone())
                                });
                                entry.insert(Some((
                                    file_lock,
                                    InputTmdbIndexTree::Series(tree),
                                )));
                                return tmdb_id;
                            }
                        }
                        PlaylistItemType::Video => {
                            if let Ok(tree) =
                                BPlusTree::<u32, InputVodInfoRecord>::load(&tmdb_path)
                            {
                                let tmdb_id = tree.query(&pid).map(|vod_record| {
                                    InputTmdbIndexValue::Video(vod_record.clone())
                                });
                                entry.insert(Some((
                                    file_lock,
                                    InputTmdbIndexTree::Video(tree),
                                )));
                                return tmdb_id;
                            }
                        }
                        _ => {}
                    }
                };
            }
            entry.insert(None);
            None
        }
    }
}

pub fn strm_get_file_paths(target_path: &Path) -> PathBuf {
    target_path.join(PathBuf::from(format!("{FILE_STRM}.{FILE_SUFFIX_DB}")))
}

#[derive(Serialize)]
struct StrmItemInfo {
    group: String,
    title: String,
    item_type: PlaylistItemType,
    provider_id: Option<u32>,
    virtual_id: u32,
    input_name: String,
    url: String,
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

fn extract_item_info(pli: &mut PlaylistItem) -> StrmItemInfo {
    let header = &mut pli.header;
    let group = header.group.to_string();
    let title = header.title.to_string();
    let item_type = header.item_type;
    let provider_id = header.get_provider_id();
    let virtual_id = header.virtual_id;
    let input_name = header.input_name.to_string();
    let url = header.url.to_string();
    let (series_name, release_date, added, season, episode) = match header.item_type {
        PlaylistItemType::Series => {
            let series_name = match header.get_field("name") {
                Some(name) if !name.is_empty() => Some(name.to_string()),
                _ => header.get_additional_property_as_str("series_name"),
            };
            let release_date = header
                .get_additional_property_as_str("series_release_date")
                .or_else(|| header.get_additional_property_as_str("release_date"));
            let season = header.get_additional_property_as_str("season");
            let episode = header.get_additional_property_as_str("episode");
            let added = header.get_additional_property_as_u64("added");
            (series_name, release_date, added, season, episode)
        }
        PlaylistItemType::Video => {
            let name = header.get_field("name").map(|v| v.to_string());
            let release_date = header.get_additional_property_as_str("release_date");
            let added = header.get_additional_property_as_u64("added");
            (name, release_date, added, None, None)
        }
        _ => (None, None, None, None, None),
    };
    StrmItemInfo {
        group,
        title,
        item_type,
        provider_id,
        virtual_id,
        input_name,
        url,
        series_name,
        release_date,
        season,
        episode,
        added,
    }
}

async fn prepare_strm_output_directory(path: &Path) -> Result<(), M3uFilterError> {
    // Ensure the directory exists
    if let Err(e) = tokio::fs::create_dir_all(path).await {
        error!("Failed to create directory {path:?}: {e}");
        return create_m3u_filter_error_result!(
            M3uFilterErrorKind::Notify,
            "Error creating STRM directory: {e}"
        );
    }
    Ok(())
}

async fn read_files_non_recursive(path: &Path) -> tokio::io::Result<Vec<PathBuf>> {
    let mut stack = vec![PathBuf::from(path)]; // Initialize the stack with the starting directory
    let mut files = vec![]; // To store all the found files

    while let Some(current_dir) = stack.pop() {
        // Read the directory
        let mut dir_read = tokio::fs::read_dir(&current_dir).await?;
        // Iterate over the entries in the current directory
        while let Some(entry) = dir_read.next_entry().await? {
            let entry_path = entry.path();
            // If it's a directory, push it onto the stack for later processing
            if entry_path.is_dir() {
                stack.push(entry_path.clone());
            } else {
                // If it's a file, add it to the entries list
                files.push(entry_path);
            }
        }
    }
    Ok(files)
}

async fn cleanup_strm_output_directory(
    cleanup: bool,
    root_path: &Path,
    existing: &HashSet<String>,
    processed: &HashSet<String>,
) -> Result<(), String> {
    if !(root_path.exists() && root_path.is_dir()) {
        return Err(format!(
            "Error: STRM directory does not exist: {root_path:?}"
        ));
    }

    let to_remove: HashSet<String> = if cleanup {
        // Remove al files which are not in `processed`
        let mut found_files = HashSet::new();
        let files = read_files_non_recursive(root_path).await.map_err(|err| err.to_string())?;
        for file_path in files {
            if let Some(file_name) = file_path
                .strip_prefix(root_path)
                .ok()
                .and_then(|p| p.to_str()) {
                found_files.insert(file_name.to_string());
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
            error!("Failed to remove file {file_path:?}: {err}");
        }
    }

    // TODO should we delete all empty directories if cleanup=false ?
    remove_empty_dirs(root_path.into()).await;
    Ok(())
}

fn filter_strm_item(pli: &PlaylistItem) -> bool {
    let item_type = pli.header.item_type;
    item_type == PlaylistItemType::Series
        || item_type == PlaylistItemType::Live
        || item_type == PlaylistItemType::Video
}

fn get_relative_path_str(full_path: &Path, root_path: &Path) -> String {
    full_path
        .strip_prefix(root_path)
        .map_or_else(
            |_| full_path.to_string_lossy(),
            |relative| relative.to_string_lossy(),
        )
        .to_string()
}

struct StrmFile {
    file_name: Arc<String>,
    dir_path: PathBuf,
    strm_info: StrmItemInfo,
}

async fn prepare_strm_files(
    cfg: &Config,
    new_playlist: &mut [PlaylistGroup],
    root_path: &Path,
    underscore_whitespace: bool,
    kodi_style: bool,
) -> Vec<StrmFile> {
    let channel_count = new_playlist
        .iter()
        .map(|g| g.filter_count(filter_strm_item))
        .sum();
    // contains all filenames to detect collisions
    let mut all_filenames = HashSet::with_capacity(channel_count);
    // contains only collision filenames
    let mut collisions: HashSet<Arc<String>> = HashSet::new();
    let mut input_tmdb_indexes: InputTmdbIndexMap = HashMap::with_capacity(channel_count);
    let mut result = Vec::with_capacity(channel_count);

    // first we create the names to identify name collisions
    for pg in new_playlist.iter_mut() {
        for pli in pg.channels.iter_mut().filter(|c| filter_strm_item(c)) {
            let strm_item_info = extract_item_info(pli);
            let (dir_path, strm_file_name) = if kodi_style {
                kodi_style_rename(
                    cfg,
                    &strm_item_info,
                    &KODI_STYLE,
                    &mut input_tmdb_indexes,
                    underscore_whitespace,
                ).await
            } else {
                let dir_path = root_path.join(sanitize_for_filename(
                    &strm_item_info.group,
                    underscore_whitespace,
                ));
                let strm_file_name =
                    sanitize_for_filename(&strm_item_info.title, underscore_whitespace);
                (dir_path, strm_file_name)
            };
            let filename = Arc::new(strm_file_name);
            if all_filenames.contains(&filename) {
                collisions.insert(Arc::clone(&filename));
            }
            all_filenames.insert(Arc::clone(&filename));
            result.push(StrmFile {
                file_name: Arc::clone(&filename),
                dir_path,
                strm_info: strm_item_info,
            });
        }
    }

    if !collisions.is_empty() {
        let separator = if underscore_whitespace { "_" } else { " " };
        result
            .iter_mut()
            .filter(|s| collisions.contains(&s.file_name))
            .for_each(|s| {
                s.file_name = Arc::new(format!(
                    "{}{separator}-{separator}[{}]",
                    s.file_name, s.strm_info.virtual_id
                ));
            });
    }
    result
}

pub async fn kodi_write_strm_playlist(
    target: &ConfigTarget,
    target_output: &StrmTargetOutput,
    cfg: &Config,
    new_playlist: &mut [PlaylistGroup],
) -> Result<(), M3uFilterError> {
    if new_playlist.is_empty() {
        return Ok(());
    }

    let Some(root_path) = file_utils::get_file_path(
        &cfg.working_dir,
        Some(std::path::PathBuf::from(&target_output.directory))
    ) else {
        return Err(info_err!(format!(
            "Failed to get file path for {}",
            target_output.directory
        )));
    };

    let credentials_and_server_info = get_credentials_and_server_info(cfg, target_output.username.as_ref()).await;
    let strm_index_path =
        strm_get_file_paths(&ensure_target_storage_path(cfg, target.name.as_str())?);
    let existing_strm = {
        let _file_lock = cfg
            .file_locks
            .read_lock(&strm_index_path);
        read_strm_file_index(&strm_index_path)
            .await
            .unwrap_or_else(|_| HashSet::with_capacity(4096))
    };
    let mut processed_strm: HashSet<String> = HashSet::with_capacity(existing_strm.len());

    let mut failed = vec![];

    prepare_strm_output_directory(&root_path).await?;

    // we need to consider
    // - Live streams
    // - Xtream Series Episode (has series_name and release_date)
    // - Xtream VOD (should have year or release_date)
    // - M3u Series (TODO we dont have this currently, should be guessed through m3u parser)
    // - M3u Vod (no additional infos, need to extract from title)
    let strm_files = prepare_strm_files(
        cfg,
        new_playlist,
        &root_path,
        target_output.underscore_whitespace,
        target_output.kodi_style,
    ).await;
    for strm_file in strm_files {
        // file paths
        let output_path = root_path.join(&strm_file.dir_path);
        let file_path = output_path.join(format!("{}.strm", strm_file.file_name));
        let file_exists = file_path.exists();
        let relative_file_path = get_relative_path_str(&file_path, &root_path);

        // create content
        let url = get_strm_url(credentials_and_server_info.as_ref(), &strm_file.strm_info);
        let mut content = target_output.strm_props.as_ref().map_or_else(Vec::new, std::clone::Clone::clone);
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
        if !ensure_strm_file_directory(&mut failed, &output_path).await {
            continue;
        }

        match write_strm_file(
            &file_path,
            content_as_bytes,
            strm_file.strm_info.get_file_ts(),
        ).await
        {
            Ok(()) => {
                processed_strm.insert(relative_file_path);
            }
            Err(err) => {
                failed.push(err);
            }
        };
    }

    if let Err(err) = write_strm_index_file(cfg, &processed_strm, &strm_index_path).await {
        failed.push(err);
    }

    if let Err(err) =
        cleanup_strm_output_directory(target_output.cleanup, &root_path, &existing_strm, &processed_strm).await
    {
        failed.push(err);
    }

    if failed.is_empty() {
        Ok(())
    } else {
        Err(info_err!(failed.join(", ")))
    }
}
async fn write_strm_index_file(
    cfg: &Config,
    entries: &HashSet<String>,
    index_file_path: &PathBuf,
) -> Result<(), String> {
    let _file_lock = cfg
        .file_locks
        .write_lock(index_file_path);
    let file = File::create(index_file_path)
        .await
        .map_err(|err| format!("Failed to create strm index file: {index_file_path:?} {err}"))?;
    let mut writer = BufWriter::new(file);
    let new_line = "\n".as_bytes();
    for entry in entries {
        writer
            .write_all(entry.as_bytes())
            .await
            .map_err(|err| format!("Failed to write strm index entry: {err}"))?;
        writer
            .write(new_line)
            .await
            .map_err(|err| format!("Failed to write strm index entry: {err}"))?;
    }
    writer
        .flush()
        .await
        .map_err(|err| format!("failed to write strm index entry: {err}"))?;
    Ok(())
}

async fn ensure_strm_file_directory(failed: &mut Vec<String>, output_path: &Path) -> bool {
    if !output_path.exists() {
        if let Err(e) = create_dir_all(output_path).await {
            let err_msg =
                format!("Failed to create directory for strm playlist: {output_path:?} {e}");
            error!("{err_msg}");
            failed.push(err_msg);
            return false; // skip creation, could not create directory
        };
    }
    true
}

async fn write_strm_file(
    file_path: &Path,
    content_as_bytes: &[u8],
    timestamp: Option<u64>,
) -> Result<(), String> {
    File::create(file_path)
        .await
        .map_err(|err| format!("Failed to create strm file: {err}"))?
        .write_all(content_as_bytes)
        .await
        .map_err(|err| format!("Failed to write strm playlist: {err}"))?;

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
        match reader.read_to_end(&mut buffer).await {
            Ok(_) => {
                let file_hash = hash_bytes(&buffer);
                if content_hash == file_hash {
                    return true;
                }
            }
            Err(err) => {
                error!("Could not read existing strm file {file_path:?} {err}");
            }
        }
    }
    false
}

async fn get_credentials_and_server_info(
    cfg: &Config,
    username: Option<&String>,
) -> Option<(ProxyUserCredentials, ApiProxyServerInfo)> {
    let username = username?;
    let credentials = cfg.get_user_credentials(username).await?;
    if credentials.proxy != ProxyType::Reverse {
        return None;
    }
    let server_info = cfg.get_user_server_info(&credentials).await;
    Some((credentials, server_info))
}

async fn read_strm_file_index(strm_file_index_path: &Path) -> std::io::Result<HashSet<String>> {
    let file = File::open(strm_file_index_path).await?;
    let reader = BufReader::new(file);
    let mut result = HashSet::new();
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        result.insert(line);
    }
    Ok(result)
}

fn get_strm_url(
    credentials_and_server_info: Option<&(ProxyUserCredentials, ApiProxyServerInfo)>,
    str_item_info: &StrmItemInfo,
) -> String {
    credentials_and_server_info.as_ref().map_or_else(
        || str_item_info.url.to_string(),
        |(user, server_info)| {
            if let Some(stream_type) = match str_item_info.item_type {
                PlaylistItemType::Series => Some("series"),
                PlaylistItemType::Live => Some("live"),
                PlaylistItemType::Video => Some("movie"),
                _ => None,
            } {
                let url = str_item_info.url.as_str();
                let ext = extract_extension_from_url(url)
                    .map_or_else(String::new, std::string::ToString::to_string);
                format!(
                    "{}/{stream_type}/{}/{}/{}{ext}",
                    server_info.get_base_url(),
                    user.username,
                    user.password,
                    str_item_info.virtual_id
                )
            } else {
                str_item_info.url.to_string()
            }
        },
    )
}

// /////////////////////////////////////////////
// - Cleanup -
// We first build a Directory Tree to
// identifiy the deletable files and directories
// /////////////////////////////////////////////
#[derive(Debug, Clone)]
struct DirNode {
    path: PathBuf,
    is_root: bool, // is root -> not delete!
    has_files: bool, //  has content -> do not delete!
    children: HashSet<PathBuf>,
    parent: Option<PathBuf>,
}

impl DirNode {
    fn new(path: PathBuf, parent: Option<PathBuf>) -> Self {
        Self::new_with_flag(path, parent, false)
    }

    fn new_root(path: PathBuf) -> Self {
        Self::new_with_flag(path, None, true)
    }

    fn new_with_flag(path: PathBuf, parent: Option<PathBuf>, is_root: bool) -> Self {
        Self {
            path,
            is_root,
            has_files: false,
            children: HashSet::new(),
            parent,
        }
    }
}

/// Because of rust ownership we don't want to use References or Mutexes.
/// Because of async operations ve cant use recursion.
/// We use paths identifier to handle the tree construction.
/// Rust sucks!!!
async fn build_directory_tree(root_path: &Path) -> HashMap<PathBuf, DirNode> {
    let mut nodes: HashMap<PathBuf, DirNode> = HashMap::new();
    nodes.insert(PathBuf::from(root_path), DirNode::new_root(root_path.to_path_buf()));
    let mut stack = vec![root_path.to_path_buf()];
    while let Some(current_path) = stack.pop() {
        if let Ok(mut dir_read) = tokio::fs::read_dir(&current_path).await {
            while let Ok(Some(entry)) = dir_read.next_entry().await {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    if !nodes.contains_key(&entry_path) {
                        let new_node = DirNode::new(entry_path.clone(), Some(current_path.clone()));
                        nodes.insert(entry_path.clone(), new_node);
                    }
                    if let Some(current_node) = nodes.get_mut(&current_path) {
                        current_node.children.insert(entry_path.clone());
                    }
                    stack.push(entry_path);
                } else if let Some(data) = nodes.get_mut(&current_path) {
                    data.has_files = true;
                    let mut parent_path_opt = data.parent.clone();

                    while let Some(parent_path) = parent_path_opt {
                        parent_path_opt = {
                            if let Some(parent) = nodes.get_mut(&parent_path) {
                                parent.has_files = true;
                                parent.parent.clone()
                            } else {
                                None
                            }
                        };
                    }
                }
            }
        }
    }
    nodes
}

// We have build the directory tree,
// now we need to build an ordered flat list,
// We walk from top to bottom.
// (PS: you can only delete in reverse order, because delete first children, then the parents)
fn flatten_tree(
    root_path: &Path,
    mut tree_nodes: HashMap<PathBuf, DirNode>,
) -> Vec<DirNode> {
    let mut paths_to_process = Vec::new(); // List of paths to process

    {
        let mut queue: VecDeque<PathBuf> = VecDeque::new(); // processing queue
        queue.push_back(PathBuf::from(root_path));

        while let Some(current_path) = queue.pop_front() {
            if let Some(current) = tree_nodes.get(&current_path) {
                current.children.iter().for_each(|child_path| {
                    if let Some(node) = tree_nodes.get(child_path) {
                        queue.push_back(node.path.clone());
                    }
                });
                paths_to_process.push(current.path.clone());
            }
        }
    }

    paths_to_process
        .iter()
        .filter_map(|path| tree_nodes.remove(path))
        .collect()
}

async fn delete_empty_dirs_from_tree(root_path: &Path, tree_nodes: HashMap<PathBuf, DirNode>) {
    let tree_stack = flatten_tree(root_path, tree_nodes);
    // reverse order  to delete from leaf to root
    for node in tree_stack.into_iter().rev() {
        if !node.has_files && !node.is_root {
            if let Err(err) = remove_dir(&node.path).await {
                trace!("Could not delete empty dir: {:?}, {err}", &node.path);
            }
        }
    }
}
async fn remove_empty_dirs(root_path: PathBuf) {
    let tree_nodes = build_directory_tree(&root_path).await;
    delete_empty_dirs_from_tree(&root_path, tree_nodes).await;
}


// #[cfg(test)]
// mod tests {
//     use crate::repository::kodi_repository::remove_empty_dirs;
//     use std::path::PathBuf;
//
//     #[tokio::test]
//     async fn test_empty_dirs() {
//         remove_empty_dirs(PathBuf::from("/tmp/hello")).await;
//     }
// }