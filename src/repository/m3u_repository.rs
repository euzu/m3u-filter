use std::fs::File;
use std::io::{BufWriter, Error, ErrorKind, Write};
use std::path::{Path, PathBuf};
use log::error;

use crate::{create_m3u_filter_error, info_err};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::{ProxyUserCredentials};
use crate::model::config::{Config, ConfigTarget};
use crate::model::playlist::{M3uPlaylistItem, PlaylistGroup, PlaylistItem, PlaylistItemType};
use crate::repository::indexed_document::{IndexedDocumentDirectAccess, IndexedDocumentWriter};
use crate::repository::m3u_playlist_iterator::M3uPlaylistIterator;
use crate::repository::storage::{FILE_SUFFIX_DB, FILE_SUFFIX_INDEX};
use crate::utils::file_utils;

const FILE_M3U: &str = "m3u";
macro_rules! cant_write_result {
    ($path:expr, $err:expr) => {
        create_m3u_filter_error!(M3uFilterErrorKind::Notify, "failed to write m3u playlist: {} - {}", $path.to_str().unwrap() ,$err)
    }
}

pub fn m3u_get_file_paths(target_path: &Path) -> (PathBuf, PathBuf) {
    let m3u_path = target_path.join(PathBuf::from(format!("{FILE_M3U}.{FILE_SUFFIX_DB}")));
    let index_path = target_path.join(PathBuf::from(format!("{FILE_M3U}.{FILE_SUFFIX_INDEX}")));
    (m3u_path, index_path)
}

pub fn m3u_get_epg_file_path(target_path: &Path) -> PathBuf {
    let path = target_path.join(PathBuf::from(format!("{FILE_M3U}.{FILE_SUFFIX_DB}")));
    file_utils::add_prefix_to_filename(&path, "epg_", Some("xml"))
}

fn persist_m3u_playlist_as_text(target: &ConfigTarget, cfg: &Config, m3u_playlist: &Vec<M3uPlaylistItem>) {
    if let Some(filename) = target.get_m3u_filename() {
        if let Some(m3u_filename) = file_utils::get_file_path(&cfg.working_dir, Some(PathBuf::from(filename))) {
            match File::create(&m3u_filename) {
                Ok(file) => {
                    let mut buf_writer = BufWriter::new(file);
                    let _ = buf_writer.write(b"#EXTM3U\n");
                    for m3u in m3u_playlist {
                        let _ = buf_writer.write(m3u.to_m3u(target.options.as_ref(), None).as_bytes());
                        let _ = buf_writer.write(b"\n");
                    }
                }
                Err(_) => {
                    error!("Can't write m3u plain playlist {}", &m3u_filename.to_str().unwrap());
                }
            }
        }
    }
}

pub async fn m3u_write_playlist(target: &ConfigTarget, cfg: &Config, target_path: &Path, new_playlist: &[PlaylistGroup]) -> Result<(), M3uFilterError> {
    if !new_playlist.is_empty() {
        let (m3u_path, idx_path) = m3u_get_file_paths(target_path);
        let m3u_playlist = new_playlist.iter()
            .flat_map(|pg| &pg.channels)
            .filter(|&pli| pli.header.borrow().item_type != PlaylistItemType::SeriesInfo)
            .map(PlaylistItem::to_m3u).collect::<Vec<M3uPlaylistItem>>();

        persist_m3u_playlist_as_text(target, cfg, &m3u_playlist);
        {
            let _file_lock = cfg.file_locks.write_lock(&m3u_path).await.map_err(|err| info_err!(format!("{err}")))?;
            match IndexedDocumentWriter::new(m3u_path.clone(), idx_path) {
                Ok(mut writer) => {
                    for m3u in m3u_playlist {
                        match writer.write_doc(m3u.virtual_id, &m3u) {
                            Ok(()) => {}
                            Err(err) => return Err(cant_write_result!(&m3u_path, err))
                        }
                    }
                    writer.store().map_err(|err| cant_write_result!(&m3u_path, err))?;
                }
                Err(err) => return Err(cant_write_result!(&m3u_path, err))
            }
        }
    }
    Ok(())
}

pub async fn m3u_load_rewrite_playlist(
    cfg: &Config,
    target: &ConfigTarget,
    user: &ProxyUserCredentials,
) -> Result<Box<dyn Iterator<Item = String>>, M3uFilterError> {
    Ok(Box::new(M3uPlaylistIterator::new(cfg, target, user).await?))
}


pub async  fn m3u_get_item_for_stream_id(cfg: &Config, stream_id: u32, m3u_path: &Path, idx_path: &Path) -> Result<M3uPlaylistItem, Error> {
    if stream_id < 1 {
        return Err(Error::new(ErrorKind::Other, "id should start with 1"));
    }
    {
        let _file_lock = cfg.file_locks.read_lock(m3u_path).await?;
        IndexedDocumentDirectAccess::read_indexed_item::<u32, M3uPlaylistItem>(m3u_path, idx_path, &stream_id)
    }
}