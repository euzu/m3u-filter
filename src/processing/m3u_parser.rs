use std::borrow::{BorrowMut};
use std::cell::RefCell;
use std::rc::Rc;
use crate::model::config::Config;
use crate::model::model_config::default_as_empty_rc_str;
use crate::model::model_playlist::{default_playlist_item_type, default_stream_cluster, PlaylistGroup, PlaylistItem, PlaylistItemHeader, PlaylistItemType, XtreamCluster};

fn token_value(it: &mut std::str::Chars) -> String {
    if let Some(oc) = it.next() {
        if oc == '"' {
            return get_value(it);
        }
    }
    String::from("")
}

fn get_value(it: &mut std::str::Chars) -> String {
    let mut result: Vec<char> = vec![];
    for oc in it.by_ref() {
        if oc == '"' {
            break;
        }
        result.push(oc);
    }
    result.iter().collect::<String>()
}

fn token_till(it: &mut std::str::Chars, stop_char: char) -> Option<String> {
    let mut result: Vec<char> = vec![];
    for ch in it.by_ref() {
        if ch == stop_char {
            break;
        } else if ch.is_whitespace() && result.is_empty() {
            continue;
        } else {
            result.push(ch);
        }
    }
    if !result.is_empty() { Some(result.iter().collect::<String>()) } else { None }
}

fn skip_digit(it: &mut std::str::Chars) -> Option<char> {
    loop {
        match it.next() {
            Some(c) => {
                if !(c == '-' || c == '+' || c.is_ascii_digit()) {
                    return Some(c);
                }
            }
            None => return None,
        }
    }
}

fn create_empty_playlistitem_header(content: &String, url: String) -> PlaylistItemHeader {
    PlaylistItemHeader {
        id: default_as_empty_rc_str(),
        stream_id: default_as_empty_rc_str(),
        name: default_as_empty_rc_str(),
        logo: default_as_empty_rc_str(),
        logo_small: default_as_empty_rc_str(),
        group: default_as_empty_rc_str(),
        title: default_as_empty_rc_str(),
        parent_code: default_as_empty_rc_str(),
        audio_track: default_as_empty_rc_str(),
        time_shift: default_as_empty_rc_str(),
        rec: default_as_empty_rc_str(),
        source: Rc::new(content.to_owned()),
        url: Rc::new(url),
        epg_channel_id: None,
        item_type: default_playlist_item_type(),
        xtream_cluster: default_stream_cluster(),
        additional_properties: None,
        series_fetched: false,
    }
}

macro_rules! process_header_fields {
    ($header:expr, $token:expr, $(($prop:ident, $field:expr)),*; $val:expr) => {
        match $token {
            $(
               $field => $header.$prop = Rc::new($val),
             )*
            _ => {}
        }
    };
}

fn process_header(video_suffixes: &Vec<&str>, content: &String, url: String) -> PlaylistItemHeader {
    let mut plih = create_empty_playlistitem_header(content, url.clone());
    let mut it = content.chars();
    let line_token = token_till(&mut it, ':');
    if line_token == Some(String::from("#EXTINF")) {
        let mut c = skip_digit(&mut it);
        loop {
            if c.is_none() {
                break;
            }
            match c.unwrap() {
                ',' => plih.title = Rc::new(get_value(&mut it)),
                _ => {
                    let token = token_till(&mut it, '=');
                    if let Some(t) = token {
                        let value = token_value(&mut it);
                        process_header_fields!(plih, t.as_str(),
                        (id, "tvg-id"),
                        (group, "group-title"),
                        (name, "tvg-name"),
                        (parent_code, "parent-code"),
                        (audio_track, "audio-track"),
                        (logo, "tvg-logo"),
                        (logo_small, "tvg-logo-small"),
                        (time_shift, "timeshift"),
                        (rec, "tvg-rec"); value)
                    }
                }
            }
            c = it.next();
        }
        if plih.group.is_empty() {
            plih.group = Rc::new(String::from("Unknown"));
        }
        plih.stream_id = Rc::clone(&plih.id);
        plih.epg_channel_id = Some(Rc::clone(&plih.id));
    }

    for suffix in video_suffixes {
        if url.ends_with(suffix) {
            // TODO find Series based on group or configured names
            plih.xtream_cluster = XtreamCluster::Video;
            plih.item_type = PlaylistItemType::Movie;
            break;
        }
    }

    let header = plih.borrow_mut();
    if header.name.is_empty() {
        if !header.title.is_empty() {
            header.name = header.title.clone();
        } else if !header.id.is_empty() {
            header.name = header.id.clone();
            header.title = header.id.clone();
        }
    }
    plih
}


pub(crate) fn parse_m3u(cfg: &Config, lines: &Vec<String>) -> Vec<PlaylistGroup> {
    let mut groups: std::collections::HashMap<Rc<String>, Vec<PlaylistItem>> = std::collections::HashMap::new();
    let mut sort_order: Vec<Rc<String>> = vec![];
    let mut header: Option<String> = None;
    let mut group: Option<String> = None;

    let video_suffixes = cfg.video.as_ref().unwrap().extensions.iter().map(|ext| ext.as_str()).collect();
    for line in lines {
        if line.starts_with("#EXTINF") {
            header = Some(String::from(line));
            continue;
        }
        if line.starts_with("#EXTGRP") {
            group = Some(String::from(&line[8..]));
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        if let Some(header_value) = header {
            let item = PlaylistItem { header: RefCell::new(process_header(&video_suffixes, &header_value, String::from(line))) };
            if let Some(group_value) = group {
                if item.header.borrow().group.is_empty() {
                    item.header.borrow_mut().group = Rc::new(group_value);
                }
            }
            let key = Rc::clone(&item.header.borrow().group);
            // let key2 = String::from(&item.header.group);
            match groups.entry(Rc::clone(&key)) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(vec![item]);
                    sort_order.push(Rc::clone(&key));
                }
                std::collections::hash_map::Entry::Occupied(mut e) => { e.get_mut().push(item); }
            }
        }
        header = None;
        group = None;
    }

    let mut result: Vec<PlaylistGroup> = vec![];
    for (grp_id, (key, channels)) in (1_u32..).zip(groups.into_iter()) {
        let cluster = channels.first().map(|pli| pli.header.borrow().xtream_cluster.clone());
        result.push(PlaylistGroup { id: grp_id, xtream_cluster: cluster.unwrap(), title: Rc::clone(&key), channels });
    }
    result.sort_by(|f, s| {
        let i1 = sort_order.iter().position(|r| **r == *f.title).unwrap();
        let i2 = sort_order.iter().position(|r| **r == *s.title).unwrap();
        i1.cmp(&i2)
    });
    result
}
