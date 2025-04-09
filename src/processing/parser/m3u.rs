use std::borrow::BorrowMut;
use crate::model::config::{Config, ConfigInput};
use crate::model::playlist::{PlaylistGroup, PlaylistItem, PlaylistItemHeader, PlaylistItemType, XtreamCluster};
use crate::processing::parser::xmltv::normalize_channel_name;
use crate::utils::hash_utils::extract_id_from_url;
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

fn create_empty_playlistitem_header(input_name: &str, url: &str) -> PlaylistItemHeader {
    PlaylistItemHeader {
        url: url.to_owned(),
        category_id: 0,
        input_name: input_name.to_string(),
        ..Default::default()
    }
}

macro_rules! process_header_fields {
    ($header:expr, $token:expr, $(($prop:ident, $field:expr)),*; $val:expr) => {
        match $token {
            $(
               $field => $header.$prop = $val,
             )*
            _ => {}
        }
    };
}

fn process_header(input: &ConfigInput, video_suffixes: &[&str], content: &str, url: &str) -> PlaylistItemHeader {
    let mut plih = create_empty_playlistitem_header(input.name.as_str(), url);
    let mut it = content.chars();
    let line_token = token_till(&mut it, ':', false);
    if line_token.as_deref() == Some("#EXTINF") {
        let mut c = skip_digit(&mut it);
        loop {
            if c.is_none() {
                break;
            }
            if c.unwrap() == ',' {
                plih.title = get_value(&mut it);
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
            let channel_id = normalize_channel_name(&plih.name);
            if let Some(chanid) = extract_id_from_url(url) {
                plih.id = chanid;
            } else {
                plih.id = channel_id.to_string();
            }
            plih.epg_channel_id = Some(channel_id);
        } else {
            plih.epg_channel_id = Some(plih.id.to_lowercase().to_string());
        }
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
            let mut item = PlaylistItem { header: process_header(input, &video_suffixes, &header_value, line) };
            let header = &mut item.header;
            if header.group.is_empty() {
                if let Some(group_value) = group {
                    header.group = group_value;
                } else {
                    let current_title = header.title.clone();
                    header.group = string_utils::get_title_group(current_title.as_str());
                }
            }
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
    let mut group_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    consume_m3u(cfg, input, lines, |item| {
        // keep the original sort order for groups and group the playlist items
        let key = {
            let header = &item.header;
            format!("{}{}", &header.xtream_cluster, &header.group)
        };
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
                                                 (pli.header.xtream_cluster, &pli.header.group)).unwrap();
        grp_id += 1;
        PlaylistGroup { id: grp_id, xtream_cluster: cluster, title: group_title.to_string(), channels }
    }).collect();
    result
}
