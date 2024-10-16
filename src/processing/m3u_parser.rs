use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::rc::Rc;

use crate::model::config::{Config, ConfigInput};
use crate::model::playlist::{PlaylistGroup, PlaylistItem, PlaylistItemHeader, PlaylistItemType, XtreamCluster};
use crate::utils::default_utils::{default_as_empty_rc_str, default_playlist_item_type, default_stream_cluster};
use crate::utils::string_utils;

fn token_value(it: &mut std::str::Chars) -> String {
    if let Some(oc) = it.next() {
        if oc == '"' {
            return get_value(it);
        }
    }
    String::new()
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

fn token_till(it: &mut std::str::Chars, stop_char: char, start_with_alpha: bool) -> Option<String> {
    let mut result: Vec<char> = vec![];
    let mut skip_non_alpha = start_with_alpha;
    for ch in it.by_ref() {
        if ch == stop_char {
            break;
        } else if ch.is_whitespace() && result.is_empty() {
            continue;
        }
        if skip_non_alpha {
            if ch.is_alphabetic() {
                skip_non_alpha = false;
            } else {
                continue;
            }
        }
        result.push(ch);
    }
    if result.is_empty() { None } else { Some(result.iter().collect::<String>()) }
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

fn create_empty_playlistitem_header(input_id: u16, url: &str) -> PlaylistItemHeader {
    PlaylistItemHeader {
        uuid: default_as_empty_rc_str(),
        id: default_as_empty_rc_str(),
        stream_id: default_as_empty_rc_str(),
        name: default_as_empty_rc_str(),
        chno: default_as_empty_rc_str(),
        logo: default_as_empty_rc_str(),
        logo_small: default_as_empty_rc_str(),
        group: default_as_empty_rc_str(),
        title: default_as_empty_rc_str(),
        parent_code: default_as_empty_rc_str(),
        audio_track: default_as_empty_rc_str(),
        time_shift: default_as_empty_rc_str(),
        rec: default_as_empty_rc_str(),
        url: Rc::new(url.to_owned()),
        epg_channel_id: None,
        item_type: default_playlist_item_type(),
        xtream_cluster: default_stream_cluster(),
        additional_properties: None,
        series_fetched: false,
        category_id: 0,
        input_id,
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

fn process_header(input: &ConfigInput, video_suffixes: &Vec<&str>, content: &str, url: &str) -> PlaylistItemHeader {
    let mut plih = create_empty_playlistitem_header(input.id, url);
    let mut it = content.chars();
    let line_token = token_till(&mut it, ':', false);
    if line_token == Some(String::from("#EXTINF")) {
        let mut c = skip_digit(&mut it);
        loop {
            if c.is_none() {
                break;
            }
            if let ',' = c.unwrap() {
                plih.title = Rc::new(get_value(&mut it));
            } else {
                let token = token_till(&mut it, '=', true);
                if let Some(t) = token {
                    let value = token_value(&mut it);
                    process_header_fields!(plih, t.to_lowercase().as_str(),
                        (id, "tvg-id"),
                        (group, "group-title"),
                        (name, "tvg-name"),
                        (chno, "tvg-chno"),
                        (parent_code, "parent-code"),
                        (audio_track, "audio-track"),
                        (logo, "tvg-logo"),
                        (logo_small, "tvg-logo-small"),
                        (time_shift, "timeshift"),
                        (rec, "tvg-rec"); value);
                }
            }
            c = it.next();
        }
        if plih.id.is_empty() {
            if let Some(chanid) = extract_id_from_url(url) {
                plih.id = Rc::new(chanid);
            }
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

pub(crate) fn extract_id_from_url(url: &str) -> Option<String> {
    if let Some(filename) = url.split('/').last() {
       return if let Some(index) = filename.rfind('.') {
            Some(filename[..index].to_string())
        } else {
            Some(filename.to_string())
        };
    }
    None
}

pub(crate) fn consume_m3u<F: FnMut(PlaylistItem)>(cfg: &Config, input: &ConfigInput, lines: impl Iterator<Item=String>, mut visit: F) {
    let mut header: Option<String> = None;
    let mut group: Option<String> = None;

    let video_suffixes = cfg.video.as_ref().unwrap().extensions.iter().map(String::as_str).collect();
    for line in lines {
        if line.starts_with("#EXTINF") {
            header = Some(line);
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
            let item = PlaylistItem { header: RefCell::new(process_header(input, &video_suffixes, &header_value, &line)) };
            let mut header = item.header.borrow_mut();
            if header.group.is_empty() {
                if let Some(group_value) = group {
                    header.group = Rc::new(group_value);
                } else {
                    let current_title = header.title.clone();
                    header.group = Rc::new(string_utils::get_title_group(current_title.as_str()));
                }
            }
            drop(header);
            visit(item);
        }
        header = None;
        group = None;
    }
}

pub(crate) fn parse_m3u(cfg: &Config, input: &ConfigInput, lines: &[String]) -> Vec<PlaylistGroup> {
    let mut sort_order: Vec<Vec<PlaylistItem>> = vec![];
    let mut idx: usize = 0;
    let mut group_map: std::collections::HashMap<Rc<String>, usize> = std::collections::HashMap::new();
    consume_m3u(cfg, input, lines.iter().cloned(), |item| {
        let key = Rc::clone(&item.header.borrow().group);
        match group_map.entry(key) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(idx);
                idx += 1;
                sort_order.push(vec![item]);
            }
            std::collections::hash_map::Entry::Occupied(o) => {
                sort_order.get_mut(*o.get()).unwrap().push(item);
            }
        }
    });

    let mut grp_id = 0;
    let result: Vec<PlaylistGroup> = sort_order.drain(..).map(|channels| {
        let channel = channels.first();
        let cluster = channel.map(|pli| pli.header.borrow().xtream_cluster).unwrap();
        let group_title = channel.map(|pli| Rc::clone(&pli.header.borrow().group)).unwrap();
        grp_id += 1;
        PlaylistGroup { id: grp_id, xtream_cluster: cluster, title: Rc::clone(&group_title), channels }
    }).collect();
    result
}
