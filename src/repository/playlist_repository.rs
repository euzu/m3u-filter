use crate::m3u_filter_error::M3uFilterError;
use crate::model::config::{Config, ConfigTarget, TargetType};
use crate::model::playlist::PlaylistGroup;
use crate::model::xmltv::Epg;
use crate::repository::epg_repository::write_epg;
use crate::repository::kodi_repository::write_strm_playlist;
use crate::repository::m3u_repository::write_m3u_playlist;
use crate::repository::xtream_repository::write_xtream_playlist;

pub(crate) fn persist_playlist(playlist: &mut [PlaylistGroup], epg: Option<Epg>,
                               target: &ConfigTarget, cfg: &Config) -> Result<(), Vec<M3uFilterError>> {
    let mut errors = vec![];
    for output in &target.output {
        match match output.target {
            TargetType::M3u => write_m3u_playlist(target, cfg, playlist, &output.filename),
            TargetType::Strm => write_strm_playlist(target, cfg, playlist, &output.filename),
            TargetType::Xtream => write_xtream_playlist(target, cfg, playlist)
        } {
            Ok(_) => {
                if !playlist.is_empty() {
                    match write_epg(target, cfg, &epg, output) {
                        Ok(_) => {}
                        Err(err) => errors.push(err)
                    }
                }
            }
            Err(err) => errors.push(err)
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}