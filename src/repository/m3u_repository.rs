use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::rc::Rc;

use chrono::Datelike;
use log::error;

use crate::{create_m3u_filter_error_result};
use crate::api::api_utils::get_user_server_info;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::UserCredentials;
use crate::model::config::{Config, ConfigTarget};
use crate::model::playlist::{PlaylistGroup, PlaylistItemType};
use crate::processing::m3u_parser::consume_m3u;
use crate::utils::file_utils;

macro_rules! cant_write_result {
    ($path:expr, $err:expr) => {
        create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write m3u playlist: {} - {}", $path.to_str().unwrap() ,$err)
    }
}

fn check_write(res: std::io::Result<()>) -> Result<(), std::io::Error> {
    match res {
        Ok(_) => Ok(()),
        Err(_) => Err(std::io::Error::new(std::io::ErrorKind::Other, "Unable to write file")),
    }
}

fn sanitize_for_filename(text: &str, underscore_whitespace: bool) -> String {
    return text.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .map(|c| if underscore_whitespace { if c.is_whitespace() { '_' } else { c } } else { c })
        .collect::<String>();
}

struct KodiStyle {
    year: regex::Regex,
    season: regex::Regex,
    episode: regex::Regex,
    whitespace: regex::Regex,
}

fn kodi_style_rename_year(name: &String, style: &KodiStyle) -> (String, Option<String>) {
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

fn kodi_style_rename_season(name: &String, style: &KodiStyle) -> (String, Option<String>) {
    match style.season.find(name) {
        Some(m) => {
            let s_season = &name[m.start()..m.end()];
            let season = Some(String::from(&s_season[1..]));
            let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
            (new_name, season)
        }
        _ => (String::from(name), Some(String::from("01"))),
    }
}

fn kodi_style_rename_episode(name: &String, style: &KodiStyle) -> (String, Option<String>) {
    match style.episode.find(name) {
        Some(m) => {
            let s_episode = &name[m.start()..m.end()];
            let episode = Some(String::from(&s_episode[1..]));
            let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
            (new_name, episode)
        }
        _ => (String::from(name), None),
    }
}

fn kodi_style_rename(name: &String, style: &KodiStyle) -> String {
    let (work_name_1, year) = kodi_style_rename_year(name, style);
    let (work_name_2, season) = kodi_style_rename_season(&work_name_1, style);
    let (work_name_3, episode) = kodi_style_rename_episode(&work_name_2, style);
    if year.is_some() && season.is_some() && episode.is_some() {
        let formatted = format!("{} ({}) S{}E{}", work_name_3, year.unwrap(), season.unwrap(), episode.unwrap());
        return String::from(style.whitespace.replace_all(formatted.as_str(), " ").as_ref());
    }
    String::from(name)
}

pub(crate) fn get_m3u_file_paths(cfg: &Config, filename: &Option<String>) -> Option<(std::path::PathBuf, std::path::PathBuf, std::path::PathBuf)> {
    match file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&filename.as_ref().unwrap()))) {
        Some(m3u_path ) => {
            let extension = m3u_path.extension().map(|ext| format!("{}_", ext.to_str().unwrap_or(""))).unwrap_or("".to_owned());
            let url_path = m3u_path.with_extension(format!("{}url", &extension));
            let index_path = m3u_path.with_extension(format!("{}idx_url", &extension));
            Some((m3u_path, url_path, index_path))
        }
        None => None
    }
}

pub(crate) fn get_m3u_epg_file_path(cfg: &Config, filename: &Option<String>) -> Option<std::path::PathBuf> {
    file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&filename.as_ref().unwrap())))
        .map(|path| file_utils::add_prefix_to_filename(&path, "epg_", Some("xml")))
}

fn create_m3u_files(m3u_path: &Path, url_path: &Path, idx_path: &Path) -> Result<(File, File, File), M3uFilterError> {
    match File::create(m3u_path) {
        Ok(m3u_file) => {
            match File::create(url_path) {
                Ok(url_file) => {
                    match File::create(idx_path) {
                        Ok(idx_file) => Ok((m3u_file, url_file, idx_file)),
                        Err(e) => cant_write_result!(&idx_path, e),
                    }
                }
                Err(e) => cant_write_result!(&url_path, e),
            }
        }
        Err(e) => cant_write_result!(&m3u_path, e),
    }
}

pub(crate) fn write_m3u_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &[PlaylistGroup], filename: &Option<String>) -> Result<(), M3uFilterError> {
    if !new_playlist.is_empty() {
        if filename.is_none() {
            return Err(M3uFilterError::new(
                M3uFilterErrorKind::Notify,
                format!("write m3u playlist for target {} failed: No filename set", target.name)));
        }
        if let Some((m3u_path, url_path, idx_path)) = get_m3u_file_paths(cfg, filename) {
            match create_m3u_files(&m3u_path, &url_path, &idx_path) {
                Ok((mut m3u_file, mut m3u_url_file, mut m3u_idx_file)) => {
                    match check_write(m3u_file.write_all(b"#EXTM3U\n")) {
                        Ok(_) => (),
                        Err(e) => return cant_write_result!(&m3u_path, e),
                    }

                    let mut id_counter: u32 = 0;
                    let mut idx_offset: u32 = 0;
                    for pg in new_playlist {
                        for pli in &pg.channels {
                            if pli.header.borrow().item_type == PlaylistItemType::SeriesInfo {
                                // we skip series info, because this is only necessary when writing xtream files
                                continue;
                            }
                            let url = pli.header.borrow().url.as_str().to_owned();
                            let url_bytes = url.as_bytes();
                            let bytes_to_write: u32 = url_bytes.len() as u32;
                            match check_write(m3u_url_file.write_all(url_bytes)) {
                                Ok(_) => {
                                    match check_write(m3u_idx_file.write_all(&idx_offset.to_le_bytes())) {
                                        Ok(_) => (),
                                        Err(e) => return cant_write_result!(&m3u_path, e),
                                    }
                                    idx_offset += bytes_to_write;
                                },
                                Err(e) => return cant_write_result!(&m3u_path, e),
                            }

                            id_counter += 1;
                            pli.header.borrow_mut().stream_id = id_counter;
                            let content = pli.to_m3u(target);
                            match check_write(m3u_file.write_all(content.as_bytes())) {
                                Ok(_) => (),
                                Err(e) => return cant_write_result!(&m3u_path, e),
                            }
                            match check_write(m3u_file.write_all(b"\n")) {
                                Ok(_) => (),
                                Err(e) => return cant_write_result!(&m3u_path, e),
                            }
                        }
                    }
                }
                Err(e) => return cant_write_result!(&m3u_path, e),
            }
        }
    }
    Ok(())
}

pub(crate) fn write_strm_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &[PlaylistGroup], filename: &Option<String>) -> Result<(), M3uFilterError> {
    if !new_playlist.is_empty() {
        if filename.is_none() {
            return Err(M3uFilterError::new(M3uFilterErrorKind::Notify, "write strm playlist failed: ".to_string()));
        }
        let underscore_whitespace = target.options.as_ref().map_or(false, |o| o.underscore_whitespace);
        let cleanup = target.options.as_ref().map_or(false, |o| o.cleanup);
        let kodi_style = target.options.as_ref().map_or(false, |o| o.kodi_style);

        if let Some(path) = file_utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&filename.as_ref().unwrap()))) {
            if cleanup {
                let _ = std::fs::remove_dir_all(&path);
            }
            if let Err(e) = std::fs::create_dir_all(&path) {
                error!("cant create directory: {:?}", &path);
                return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write strm playlist: {}", e);
            };
            for pg in new_playlist {
                for pli in &pg.channels {
                    let header = &pli.header.borrow();
                    let dir_path = path.join(sanitize_for_filename(&header.group, underscore_whitespace));
                    if let Err(e) = std::fs::create_dir_all(&dir_path) {
                        error!("cant create directory: {:?}", &path);
                        return create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write strm playlist: {}", e);
                    };
                    let mut file_name = sanitize_for_filename(&header.title, underscore_whitespace);
                    if kodi_style {
                        let style = KodiStyle {
                            season: regex::Regex::new(r"[Ss]\d\d").unwrap(),
                            episode: regex::Regex::new(r"[Ee]\d\d").unwrap(),
                            year: regex::Regex::new(r"\d\d\d\d").unwrap(),
                            whitespace: regex::Regex::new(r"\s+").unwrap(),
                        };
                        file_name = kodi_style_rename(&file_name, &style);
                    }
                    let file_path = dir_path.join(format!("{}.strm", file_name));
                    match File::create(&file_path) {
                        Ok(mut strm_file) => {
                            match check_write(strm_file.write_all(header.url.as_bytes())) {
                                Ok(_) => (),
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


pub(crate) fn rewrite_m3u_playlist(cfg: &Config, target: &ConfigTarget, user: &UserCredentials) -> Option<String> {
    let filename = target.get_m3u_filename();
    if filename.is_some() {
        if let Some((m3u_path, url_path, idx_path)) = get_m3u_file_paths(cfg, &filename) {
            if m3u_path.exists() && url_path.exists() && idx_path.exists() {
                let server_info = get_user_server_info(cfg, user);
                let url = format!("{}/m3u-stream/{}/{}", server_info.get_base_url(), user.username, user.password);
                let mut result = vec![];
                result.push("#EXTM3U".to_string());
                match File::open(&m3u_path) {
                    Ok(m3u_file) => {
                        let lines: Vec<String> = BufReader::new(m3u_file).lines().map_while(Result::ok).collect();
                        consume_m3u(cfg, &lines, |item| {
                            let stream_id = item.header.borrow().stream_id;
                            item.header.borrow_mut().url = Rc::new(format!("{}/{}", url, stream_id));
                            result.push(item.to_m3u(target))
                        });
                    }
                    Err(err) => {
                        error!("Could not open file {}: {}", &m3u_path.to_str().unwrap(), err);
                        return None;
                    }
                }
                return Some(result.join("\n"));
            };
        }
    }
    None
}