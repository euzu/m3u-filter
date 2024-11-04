use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::rc::Rc;

use crate::model::config::{Config, ConfigInput};
use crate::model::playlist::{PlaylistGroup, PlaylistItem, PlaylistItemHeader, PlaylistItemType, XtreamCluster};
use crate::utils::string_utils;

#[inline]
fn token_value(it: &mut std::str::Chars) -> String {
    // Use .find() to skip until the first double quote (") character.
    if it.any(|ch| ch == '"') {
        // If a quote is found, call get_value to extract the value.
        return get_value(it);
    }
    // If no double quote is found, return an empty string.
    String::new()
}

fn get_value(it: &mut std::str::Chars) -> String {
    let mut result = String::with_capacity(128);
    for oc in it.by_ref() {
        if oc == '"' {
            break;
        }
        result.push(oc);
    }
    result.shrink_to_fit();
    result
}

fn token_till(it: &mut std::str::Chars, stop_char: char, start_with_alpha: bool) -> Option<String> {
    let mut result = String::with_capacity(128);
    let mut skip_non_alpha = start_with_alpha;

    for ch in it.by_ref() {
        if ch == stop_char {
            break;
        }
        if result.is_empty() && ch.is_whitespace() {
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

    if result.is_empty() {
        None
    } else {
        result.shrink_to_fit();
        Some(result)
    }
}

#[inline]
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
        url: Rc::new(url.to_owned()),
        category_id: 0,
        input_id,
        ..Default::default()
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

fn process_header(input: &ConfigInput, video_suffixes: &[&str], content: &str, url: &str) -> PlaylistItemHeader {
    let mut plih = create_empty_playlistitem_header(input.id, url);
    let mut it = content.chars();
    let line_token = token_till(&mut it, ':', false);
    if line_token.as_deref() == Some("#EXTINF") {
        let mut c = skip_digit(&mut it);
        loop {
            if c.is_none() {
                break;
            }
            if c.unwrap() == ',' {
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
        // plih.virtual_id = plih.id;
        plih.epg_channel_id = Some(Rc::clone(&plih.id));
    }

    if video_suffixes.iter().any(|suffix| url.ends_with(suffix)) {
        // TODO find Series based on group or configured names
        plih.xtream_cluster = XtreamCluster::Video;
        plih.item_type = PlaylistItemType::Video;
    }

    {
        let header = plih.borrow_mut();
        if header.name.is_empty() {
            if !header.title.is_empty() {
                header.name = header.title.clone();
            } else if !header.id.is_empty() {
                header.name = header.id.clone();
                header.title = header.id.clone();
            }
        }
    }

    plih
}

pub fn extract_id_from_url(url: &str) -> Option<String> {
    if let Some(filename) = url.split('/').last() {
        return filename.rfind('.').map_or_else(|| Some(filename.to_string()), |index| Some(filename[..index].to_string()));
    }
    None
}

pub fn consume_m3u<'a, I, F: FnMut(PlaylistItem)>(cfg: &Config, input: &ConfigInput, lines: I, mut visit: F)
where
    I: Iterator<Item=&'a str>,
{
    let mut header: Option<String> = None;
    let mut group: Option<String> = None;

    let video_suffixes = cfg.video.as_ref().unwrap().extensions.iter().map(String::as_str).collect::<Vec<&str>>();
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
            let item = PlaylistItem { header: RefCell::new(process_header(input, &video_suffixes, &header_value, line)) };
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

pub fn parse_m3u<'a, I>(cfg: &Config, input: &ConfigInput, lines: I) -> Vec<PlaylistGroup>
where
    I: Iterator<Item=&'a str>,
{
    let mut sort_order: Vec<Vec<PlaylistItem>> = vec![];
    let mut sort_order_idx: usize = 0;
    let mut group_map: std::collections::HashMap<Rc<String>, usize> = std::collections::HashMap::new();
    consume_m3u(cfg, input, lines, |item| {
        // keep the original sort order for groups and group the playlist items
        let key = Rc::clone(&item.header.borrow().group);
        match group_map.entry(key) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(sort_order_idx);
                sort_order.push(vec![item]);
                sort_order_idx += 1;
            }
            std::collections::hash_map::Entry::Occupied(o) => {
                sort_order.get_mut(*o.get()).unwrap().push(item);
            }
        }
    });
    let mut grp_id = 0;
    let result: Vec<PlaylistGroup> = sort_order.into_iter().map(|channels| {
        // create a group based on the first playlist item
        let channel = channels.first();
        let (cluster, group_title) = channel.map(|pli|
                                                 (pli.header.borrow().xtream_cluster, Rc::clone(&pli.header.borrow().group))).unwrap();
        grp_id += 1;
        PlaylistGroup { id: grp_id, xtream_cluster: cluster, title: Rc::clone(&group_title), channels }
    }).collect();
    result
}
