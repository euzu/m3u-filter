use crate::m3u_filter_error::M3uFilterError;
use crate::model::config::{ConfigTarget, InputType, TargetType};
use crate::model::playlist::FetchedPlaylist;
use crate::processing::playlist_processor::ProcessingPipe;
use crate::utils::download;

pub async fn playlist_resolve_series(target: &ConfigTarget, errors: &mut Vec<M3uFilterError>,
                                     pipe: &ProcessingPipe,
                                     fpl: &mut FetchedPlaylist<'_>,
                                     new_fpl: &mut FetchedPlaylist<'_>) {
    let (resolve_series, resolve_series_delay) =
        if let Some(options) = &target.options {
            (options.xtream_resolve_series && fpl.input.input_type == InputType::Xtream && target.has_output(&TargetType::M3u),
             options.xtream_resolve_series_delay)
        } else {
            (false, 0)
        };
    if resolve_series {
        let mut series_playlist = download::get_xtream_playlist_series(fpl, errors, resolve_series_delay).await;
        // original content saved into original list
        for plg in &series_playlist {
            fpl.update_playlist(plg);
        }
        // run processing pipe over new items
        for f in pipe {
            let r = f(&mut series_playlist, target);
            if let Some(v) = r {
                series_playlist = v;
            }
        }
        // assign new items to the new playlist
        for plg in &series_playlist {
            new_fpl.update_playlist(plg);
        }
    }
}