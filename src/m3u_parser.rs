use std::cell::RefCell;
use crate::model::config::Config;
use crate::model::model_m3u::{PlaylistGroup, PlaylistItem, PlaylistItemHeader, XtreamCluster};

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

fn create_empty_playlistitem_header(content: &String) -> PlaylistItemHeader {
    PlaylistItemHeader {
        id: String::from(""),
        name: String::from(""),
        logo: String::from(""),
        logo_small: String::from(""),
        group: String::from("Unknown"),
        title: String::from(""),
        parent_code: String::from(""),
        audio_track: String::from(""),
        time_shift: String::from(""),
        rec: String::from(""),
        source: String::from(content),
        xtream_cluster: XtreamCluster::LIVE,
        additional_properties: None,
    }
}

fn process_header(video_suffixes: &Vec<&str>,content: &String, url: String) -> PlaylistItemHeader {
    let mut plih = create_empty_playlistitem_header(content);
    let mut it = content.chars();
    let line_token = token_till(&mut it, ':');
    if line_token == Some(String::from("#EXTINF")) {
        let mut c = skip_digit(&mut it);
        loop {
            if c.is_none() {
                break;
            }
            match c.unwrap() {
                ',' => plih.title = get_value(&mut it),
                _ => {
                    let token = token_till(&mut it, '=');
                    if let Some(t) = token {
                        let value = token_value(&mut it);
                        match t.as_str() {
                            "tvg-id" => plih.id = value,
                            "tvg-name" => plih.name = value,
                            "group-title" => if !value.is_empty() { plih.group = value },
                            "parent-code" => plih.parent_code = value,
                            "audio-track" => plih.audio_track = value,
                            "tvg-logo" => plih.logo = value,
                            "tvg-logo-small" => plih.logo_small = value,
                            "timeshift" => plih.time_shift = value,
                            "tvg-rec" => plih.rec = value,
                            _ => {}
                        }
                    }
                }
            }
            c = it.next();
        }
    }

    for suffix in video_suffixes {
       if url.ends_with(suffix) {
           // TODO find Series based on group or configured names
           plih.xtream_cluster = XtreamCluster::VIDEO;
           break;
       }
    }
    plih
}



pub(crate) fn parse_m3u(cfg: &Config, lines: &Vec<String>) -> Vec<PlaylistGroup> {
    let mut groups: std::collections::HashMap<String, Vec<PlaylistItem>> = std::collections::HashMap::new();
    let mut sort_order: Vec<String> = vec![];
    let mut header: Option<String> = None;
    let mut group: Option<String> = None;

    let video_suffixes = if let Some(suffixes) = &cfg.video_suffix {
        suffixes.iter().map(|s| s.as_str()).collect()
    } else {
        vec![".mp4", ".mkv", ".avi"]
    };

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
        if let Some(..) = header {
            let item = PlaylistItem { header: RefCell::new(process_header(&video_suffixes, &header.unwrap(), String::from(line))), url: String::from(line) };
            if group.is_some() && item.header.borrow().group.is_empty() {
                item.header.borrow_mut().group = group.unwrap();
            }
            let key = String::from(&item.header.borrow().group);
            // let key2 = String::from(&item.header.group);
            match groups.entry(key.clone()) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(vec![item]);
                    sort_order.push(key);
                }
                std::collections::hash_map::Entry::Occupied(mut e) => { e.get_mut().push(item); }
            }
        }
        header = None;
        group = None;
    }

    let mut result: Vec<PlaylistGroup> = vec![];
    let mut grp_id: i32 = 0;
    for (key, channels) in groups {
        grp_id += 1;
        let cluster = channels.first().map(|pli| pli.header.borrow().xtream_cluster.clone());
        result.push(PlaylistGroup { id: grp_id, xtream_cluster: cluster.unwrap(), title: key, channels });
    }
    result.sort_by(|f, s| {
        let i1 = sort_order.iter().position(|r| r == &f.title).unwrap();
        let i2 = sort_order.iter().position(|r| r == &s.title).unwrap();
        i1.cmp(&i2)
    });
    result
}
