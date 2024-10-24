use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{Config, ConfigTarget, TargetType};
use crate::model::playlist::PlaylistGroup;
use crate::model::xmltv::Epg;
use crate::repository::epg_repository::epg_write;
use crate::repository::kodi_repository::kodi_write_strm_playlist;
use crate::repository::m3u_repository::m3u_write_playlist;
use crate::repository::storage::{ensure_target_storage_path, get_target_id_mapping_file};
use crate::repository::target_id_mapping_record::TargetIdMapping;
use crate::repository::xtream_repository::xtream_write_playlist;

pub(crate) fn persist_playlist(playlist: &mut [PlaylistGroup], epg: Option<&Epg>,
                               target: &ConfigTarget, cfg: &Config) -> Result<(), Vec<M3uFilterError>> {
    let mut errors = vec![];

    // TODO get previous virtual-ids and match them to the playlist items
    match ensure_target_storage_path(cfg, target.name.as_str()) {
        Ok(target_path) => {
            let mut target_id_mapping = TargetIdMapping::from_path(&get_target_id_mapping_file(&target_path));
            for group in &mut *playlist {
                for channel in &group.channels {
                    let mut header = channel.header.borrow_mut();
                    match header.id.parse::<u32>() {
                        Ok(provider_id) => {
                            let uuid = header.get_uuid();
                            header.virtual_id = target_id_mapping.insert_entry(provider_id, **uuid);
                        }
                        Err(err) => {
                            errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()));
                        }
                    }
                }
            }

            for output in &target.output {
                match match output.target {
                    TargetType::M3u => m3u_write_playlist(target, cfg, &target_path, playlist),
                    TargetType::Xtream => xtream_write_playlist(target, cfg, &target_path, playlist),
                    TargetType::Strm => kodi_write_strm_playlist(target, cfg, playlist, &output.filename),
                } {
                    Ok(()) => {
                        if let Err(err) = target_id_mapping.to_path(&target_path) {
                            errors.push(M3uFilterError::new(M3uFilterErrorKind::Info, err.to_string()));
                        }
                        if !playlist.is_empty() {
                            match epg_write(target, cfg, &target_path, epg, output) {
                                Ok(()) => {}
                                Err(err) => errors.push(err)
                            }
                        }
                    }
                    Err(err) => errors.push(err)
                }
            }
        }
        Err(err) => errors.push(err),
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
