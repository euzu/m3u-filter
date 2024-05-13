use crate::m3u_filter_error::M3uFilterError;
use crate::model::config::{Config, ConfigTarget, TargetType};
use crate::model::playlist::PlaylistGroup;
use crate::model::xmltv::Epg;
use crate::repository::epg_repository::epg_write;
use crate::repository::kodi_repository::kodi_write_strm_playlist;
use crate::repository::m3u_repository::m3u_write_playlist;
use crate::repository::xtream_repository::xtream_write_playlist;

pub(crate) fn persist_playlist(playlist: &mut [PlaylistGroup], epg: Option<&Epg>,
                               target: &ConfigTarget, cfg: &Config) -> Result<(), Vec<M3uFilterError>> {
    let mut errors = vec![];
    for output in &target.output {
        match match output.target {
            TargetType::M3u => m3u_write_playlist(target, cfg, playlist),
            TargetType::Strm => kodi_write_strm_playlist(target, cfg, playlist, &output.filename),
            TargetType::Xtream => xtream_write_playlist(target, cfg, playlist)
        } {
            Ok(()) => {
                if !playlist.is_empty() {
                    match epg_write(target, cfg, epg, output) {
                        Ok(()) => {}
                        Err(err) => errors.push(err)
                    }
                }
            }
            Err(err) => errors.push(err)
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
