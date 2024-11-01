use log::error;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget};
use crate::model::playlist::{XtreamCluster, XtreamPlaylistItem};
use crate::model::xtream::XtreamMappingOptions;
use crate::repository::indexed_document::IndexedDocumentReader;
use crate::repository::xtream_repository::{xtream_get_file_paths, xtream_get_storage_path};
use crate::utils::file_lock_manager::FileReadGuard;

pub(crate) struct XtreamPlaylistIterator {
    reader: IndexedDocumentReader<XtreamPlaylistItem>,
    options: XtreamMappingOptions,
    category_id: u32,
    _file_lock: FileReadGuard,
}

impl XtreamPlaylistIterator {
    pub fn new(
        cluster: XtreamCluster,
        config: &Config,
        target: &ConfigTarget,
        category_id: u32,
    ) -> Result<Self, M3uFilterError> {
        if let Some(storage_path) = xtream_get_storage_path(config, target.name.as_str()) {
            let (xtream_path, idx_path) = xtream_get_file_paths(&storage_path, cluster);
            let file_lock = config.file_locks.read_lock(&xtream_path).map_err(|err|
            M3uFilterError::new(M3uFilterErrorKind::Info, format!("Could not lock document {xtream_path:?}: {err}"))
            )?;

            let reader = IndexedDocumentReader::<XtreamPlaylistItem>::new(&xtream_path, &idx_path)
                .map_err(|err| M3uFilterError::new(M3uFilterErrorKind::Info, format!("Could not deserialize file {} - {}", &xtream_path.to_str().unwrap(), err)))?;

            let options = XtreamMappingOptions::from_target_options(target.options.as_ref());

            Ok(Self {
                reader,
                options,
                category_id,
                _file_lock: file_lock,
            })
        } else {
            Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Failed to find xtream storage for target {}", &target.name)))
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
        self.reader.find(|pli| self.category_id == 0 || pli.category_id == self.category_id)
            .map(|pli| pli.to_doc(&self.options).to_string())
    }
}