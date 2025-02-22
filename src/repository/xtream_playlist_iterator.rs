use std::collections::HashSet;
use log::error;
use crate::m3u_filter_error::info_err;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::{ProxyUserCredentials};
use crate::model::config::{Config, ConfigTarget};
use crate::model::playlist::{XtreamCluster, XtreamPlaylistItem};
use crate::model::xtream::XtreamMappingOptions;
use crate::repository::indexed_document::{IndexedDocumentIterator};
use crate::repository::user_repository::user_get_bouquet_filter;
use crate::repository::xtream_repository::{xtream_get_file_paths, xtream_get_storage_path};
use crate::utils::file::file_lock_manager::FileReadGuard;

pub struct XtreamPlaylistIterator {
    reader: IndexedDocumentIterator<u32, XtreamPlaylistItem>,
    options: XtreamMappingOptions,
    filter: Option<HashSet<String>>,
    _file_lock: FileReadGuard,
    base_url: String,
    user: ProxyUserCredentials,
}

impl XtreamPlaylistIterator {
    pub async fn new(
        cluster: XtreamCluster,
        config: &Config,
        target: &ConfigTarget,
        category_id: u32,
        user: &ProxyUserCredentials
    ) -> Result<Self, M3uFilterError> {
        if let Some(storage_path) = xtream_get_storage_path(config, target.name.as_str()) {
            let (xtream_path, idx_path) = xtream_get_file_paths(&storage_path, cluster);
            if !xtream_path.exists() || !idx_path.exists() {
                return Err(info_err!(format!("No {cluster} entries found for target {}", &target.name)));
            }
            let file_lock = config.file_locks.read_lock(&xtream_path);

            let reader = IndexedDocumentIterator::<u32, XtreamPlaylistItem>::new(&xtream_path, &idx_path)
                .map_err(|err| info_err!(format!("Could not deserialize file {xtream_path:?} - {err}")))?;

            let options = XtreamMappingOptions::from_target_options(target.options.as_ref(), config);
            let server_info = config.get_user_server_info(user);

            let category_id_str = if category_id == 0 { String::new() }  else { category_id.to_string() };
            let filter = user_get_bouquet_filter(config, &user.username, &category_id_str, cluster).await;

            Ok(Self {
                reader,
                options,
                filter,
                _file_lock: file_lock,
                base_url: server_info.get_base_url(),
                user: user.clone(),
            })
        } else {
            Err(info_err!(format!("Failed to find xtream storage for target {}", &target.name)))
        }
    }
}

impl Iterator for XtreamPlaylistIterator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.reader.has_error() {
            error!("Could not deserialize xtream item: {:?}", self.reader.get_path());
            return None;
        }
        if let Some(set) = &self.filter {
            self.reader
                .find(|pli| set.contains(&pli.category_id.to_string()))
                .map(|pli| pli.to_doc(&self.base_url, &self.options, &self.user).to_string())
        } else {
            self.reader
                .next()
                .map(|pli| pli.to_doc(&self.base_url, &self.options, &self.user).to_string())
        }
    }
}