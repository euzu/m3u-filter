use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget, TargetType};
use crate::model::playlist::PlaylistGroup;
use crate::model::xmltv::Epg;
use crate::repository::epg_repository::epg_write;
use crate::repository::kodi_repository::kodi_write_strm_playlist;
use crate::repository::m3u_repository::m3u_write_playlist;
use crate::repository::storage::{ensure_target_storage_path, get_target_id_mapping_file};
use crate::repository::target_id_mapping::TargetIdMapping;
use crate::repository::xtream_repository::xtream_write_playlist;

pub(crate) fn persist_playlist(playlist: &mut [PlaylistGroup], epg: Option<&Epg>,
                               target: &ConfigTarget, cfg: &Config) -> Result<(), Vec<M3uFilterError>> {
    let mut errors = vec![];
    let target_path = match ensure_target_storage_path(cfg, &target.name) {
        Ok(path) => path,
        Err(err) => return Err(vec![err]),
    };

    let target_id_mapping_file = get_target_id_mapping_file(&target_path);
    let _file_lock = match cfg.file_locks.write_lock(&target_id_mapping_file) {
        Ok(lock) => lock,
        Err(err) => {
            errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()));
            return Err(errors);
        }
    };

    let mut target_id_mapping = TargetIdMapping::new(&target_id_mapping_file);

    // Virtual IDs assignment
    for group in playlist.iter_mut() {
        for channel in &group.channels {
            let mut header = channel.header.borrow_mut();
            if let Some(provider_id) = header.get_provider_id() {
                let uuid = header.get_uuid();
                let item_type = header.item_type;
                header.virtual_id = target_id_mapping.insert_entry(**uuid, provider_id, item_type, 0);
            } else {
                errors.push(M3uFilterError::new(
                    M3uFilterErrorKind::Info,
                    format!("Playlist item has no provider id: {}", &header.title),
                ));
            }
        }
    }

    for output in &target.output {
        let result = match output.target {
            TargetType::M3u => m3u_write_playlist(target, cfg, &target_path, playlist),
            TargetType::Xtream => xtream_write_playlist(target, cfg, playlist),
            TargetType::Strm => kodi_write_strm_playlist(target, cfg, playlist, &output.filename),
        };

        if let Err(err) = result {
            errors.push(err);
        } else {
            if let Err(err) = target_id_mapping.persist() {
                errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()));
            }
            if !playlist.is_empty() {
                if let Err(err) = epg_write(target, cfg, &target_path, epg, output) {
                    errors.push(err);
                }
            }
        }
    }

    if let Err(err) = target_id_mapping.persist() {
        errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()));
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }

}
