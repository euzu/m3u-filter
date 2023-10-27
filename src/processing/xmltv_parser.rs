use log::debug;
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::xmltv::TVGuide;

pub(crate) fn parse_tvguide(content: &str) -> (Option<TVGuide>, Vec<M3uFilterError>) {
    match serde_xml_rs::from_str::<TVGuide>(content) {
        Ok(mut tv_guide) => {
            tv_guide.prepare();
            (Some(tv_guide), vec![])
        }
        Err(err) => (None, vec![M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Failed to download epg: {}", err))])
    }
}

pub(crate) fn flatten_tvguide(tvguides: &mut Vec<TVGuide>) -> Option<TVGuide> {
    if tvguides.is_empty() {
        debug!("tvguides are empty");
        None
    } else if tvguides.len() == 1 {
        Some(tvguides.remove(0))
    } else {
        let mut guide = TVGuide {
            channels: vec![],
            programs: vec![],
            date: None,
            source_info_url: None,
            source_info_name: None,
        };

        tvguides.drain(..).for_each(|mut g| {
            if guide.date.is_none() {
                guide.date = g.date.to_owned();
            }
            if guide.source_info_url.is_none() {
                guide.source_info_url = g.source_info_url.to_owned();
            }
            if guide.source_info_name.is_none() {
                guide.source_info_name = g.source_info_name.to_owned();
            }
            g.channels.drain(..).for_each(|c| guide.channels.push(c));
            g.programs.drain(..).for_each(|c| guide.programs.push(c));
        });

        Some(guide)
    }
}