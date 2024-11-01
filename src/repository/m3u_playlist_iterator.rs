use crate::api::api_utils::get_user_server_info;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::api_proxy::{ProxyType, ProxyUserCredentials};
use crate::model::config::{Config, ConfigTarget, ConfigTargetOptions};
use crate::model::playlist::M3uPlaylistItem;
use crate::repository::indexed_document::IndexedDocumentReader;
use crate::repository::m3u_repository::m3u_get_file_paths;
use crate::repository::storage::ensure_target_storage_path;
use crate::utils::file_lock_manager::FileReadGuard;

pub(crate) struct M3uPlaylistIterator {
    reader: IndexedDocumentReader<M3uPlaylistItem>,
    base_url: String,
    target_options: Option<ConfigTargetOptions>,
    proxy_type: ProxyType,
    _file_lock: FileReadGuard,
    started: bool,
}

impl M3uPlaylistIterator {
    pub fn new(
        cfg: &Config,
        target: &ConfigTarget,
        user: &ProxyUserCredentials,
    ) -> Result<Self, M3uFilterError> {
        let target_path = ensure_target_storage_path(cfg, target.name.as_str())?;
        let (m3u_path, idx_path) = m3u_get_file_paths(&target_path);

        let file_lock = cfg.file_locks.read_lock(&m3u_path).map_err(|err|
            M3uFilterError::new(M3uFilterErrorKind::Info, format!("Could not lock document {m3u_path:?}: {err}"))
        )?;

        let reader = IndexedDocumentReader::<M3uPlaylistItem>::new(&m3u_path, &idx_path).map_err(|err|
            M3uFilterError::new(M3uFilterErrorKind::Info, format!("Could not deserialize file {m3u_path:?} - {err}")))?;

        let server_info = get_user_server_info(cfg, user);
        let base_url = format!(
            "{}/m3u-stream/{}/{}",
            server_info.get_base_url(), user.username, user.password
        );

        Ok(Self {
            reader,
            base_url,
            target_options: target.options.clone(),
            proxy_type: user.proxy.clone(),
            _file_lock: file_lock, // Speichern des Locks in der Struct
            started: false,
        })
    }
}

impl Iterator for M3uPlaylistIterator {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.started {
            self.started = true;
            return Some("#EXTM3U".to_string());
        }

        self.reader.next().map(|m3u_pli| {
            match self.proxy_type {
                ProxyType::Reverse => {
                    m3u_pli.to_m3u(&self.target_options, Some(format!("{}/{}", &self.base_url, m3u_pli.virtual_id).as_str()))
                }
                ProxyType::Redirect => {
                    m3u_pli.to_m3u(&self.target_options, None)
                }
            }
        })
    }
}