use std::fs::File;
use std::io::{BufWriter, Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use log::error;

use crate::{create_m3u_filter_error_result};
use crate::api::api_utils::get_user_server_info;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::{Config, ConfigTarget};
use crate::model::playlist::{M3uPlaylistItem, PlaylistGroup, PlaylistItem, PlaylistItemType};
use crate::repository::index_record::IndexRecord;
use crate::repository::indexed_document_reader::{IndexedDocumentReader, read_indexed_item};
use crate::repository::indexed_document_writer::IndexedDocumentWriter;
use crate::utils::file_utils;

macro_rules! cant_write_result {
    ($path:expr, $err:expr) => {
        create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "failed to write m3u playlist: {} - {}", $path.to_str().unwrap() ,$err)
    }
}

fn m3u_get_base_file_path(cfg: &Config, target: &ConfigTarget) -> Option<PathBuf> {
    file_utils::get_file_path(&cfg.working_dir, Some(PathBuf::from(format!("m3u_{}.db", target.name.replace(' ', "_").as_str()))))
}

pub(crate) fn m3u_get_file_paths(cfg: &Config, target: &ConfigTarget) -> Option<(PathBuf, PathBuf)> {
    match m3u_get_base_file_path(cfg, target) {
        Some(m3u_path) => {
            let extension = m3u_path.extension().map(|ext| format!("{}_", ext.to_str().unwrap_or("")));
            let index_path = m3u_path.with_extension(format!("{}idx", &extension.unwrap_or(String::new())));
            Some((m3u_path, index_path))
        }
        None => None
    }
}

pub(crate) fn m3u_get_epg_file_path(cfg: &Config, target: &ConfigTarget) -> Option<PathBuf> {
    m3u_get_base_file_path(cfg, target)
        .map(|path| file_utils::add_prefix_to_filename(&path, "epg_", Some("xml")))
}

pub(crate) fn m3u_write_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &[PlaylistGroup]) -> Result<(), M3uFilterError> {
    if !new_playlist.is_empty() {
        if let Some((m3u_path, idx_path)) = m3u_get_file_paths(cfg, target) {

            let m3u_playlist = new_playlist.iter()
                .flat_map(|pg| &pg.channels)
                .filter(|&pli| pli.header.borrow().item_type != PlaylistItemType::SeriesInfo)
                .map(PlaylistItem::to_m3u).collect::<Vec<M3uPlaylistItem>>();

            if let Some(filename) = target.get_m3u_filename() {
                if let Some(m3u_filename) = file_utils::get_file_path(&cfg.working_dir, Some(PathBuf::from(filename))) {
                    match File::create(&m3u_filename) {
                        Ok(file) => {
                            let mut buf_writer = BufWriter::new(file);
                            let _ = buf_writer.write("#EXTM3U\n".as_bytes());
                            for m3u in &m3u_playlist {
                                let _ = buf_writer.write(m3u.to_m3u(target, None).as_bytes());
                                let _ = buf_writer.write("\n".as_bytes());
                            }
                        }
                        Err(_) => {
                            error!("Can't write m3u plain playlist {}", &m3u_filename.to_str().unwrap());
                        }
                    }
                }
            }
            match IndexedDocumentWriter::new(m3u_path.clone(), idx_path) {
                Ok(mut writer) => {
                    let mut stream_id: u32 = 1;
                    for mut m3u in m3u_playlist {
                        m3u.stream_id = Rc::new(stream_id.to_string());
                        match writer.write_doc(&m3u) {
                            Ok(_) => stream_id += 1,
                            Err(err) => return cant_write_result!(&m3u_path, err)
                        }
                    }
                }
                Err(err) => return cant_write_result!(&m3u_path, err)
            }
        }
    }
    Ok(())
}

pub(crate) fn m3u_load_rewrite_playlist(cfg: &Config, target: &ConfigTarget, user: &ProxyUserCredentials) -> Option<String> {
    if let Some((m3u_path, idx_path)) = m3u_get_file_paths(cfg, target) {
        match IndexedDocumentReader::<M3uPlaylistItem>::new(&m3u_path, &idx_path) {
            Ok(mut reader) => {
                let server_info = get_user_server_info(cfg, user);
                let url = format!("{}/m3u-stream/{}/{}", server_info.get_base_url(), user.username, user.password);
                let mut result = vec![];
                result.push("#EXTM3U".to_string());
                for m3u_pli in reader.by_ref() {
                    match user.proxy {
                        ProxyType::Reverse => {
                            let stream_id = Rc::clone(&m3u_pli.stream_id);
                            result.push(m3u_pli.to_m3u(target, Some(format!("{url}/{stream_id}").as_str())));
                        }
                        ProxyType::Redirect => {
                            result.push(m3u_pli.to_m3u(target, None));
                        }
                    }
                };
                if reader.by_ref().has_error() {
                    error!("Could not deserialize m3u item {}", &m3u_path.to_str().unwrap());
                } else {
                    return Some(result.join("\n"));
                }
            }
            Err(err) => {
                error!("Could not deserialize file {} - {}", &m3u_path.to_str().unwrap(), err);
            }
        }
    } else {
        error!("Could not open files for target {}", &target.name);
    }
    None
}

pub(crate) fn m3u_get_item_for_stream_id(stream_id: u32, m3u_path: &Path, idx_path: &Path) -> Result<M3uPlaylistItem, Error> {
    if stream_id < 1 {
        return Err(Error::new(ErrorKind::Other, "id should start with 1"));
    }
    read_indexed_item::<M3uPlaylistItem>(m3u_path, idx_path, IndexRecord::get_index_offset(stream_id - 1))
}