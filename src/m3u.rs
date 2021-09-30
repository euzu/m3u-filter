// https://de.wikipedia.org/wiki/M3U
// https://siptv.eu/howto/playlist.html

pub struct PlaylistItemHeader {
    pub id: String,
    pub name: String,
    pub logo: String,
    pub logo_small: String,
    pub group: String,
    pub title: String,
    pub parent_code: String,
    pub audio_track: String,
    pub time_shift: String,
    pub rec: String,
    pub source: String,
}

impl Clone for PlaylistItemHeader {
    fn clone(&self) -> PlaylistItemHeader {
        PlaylistItemHeader {
            id: String::from(&self.id),
            name: String::from(&self.name),
            logo: String::from(&self.logo),
            logo_small: String::from(&self.logo_small),
            group: String::from(&self.group),
            title: String::from(&self.title),
            parent_code: String::from(&self.parent_code),
            audio_track: String::from(&self.audio_track),
            time_shift: String::from(&self.time_shift),
            rec: String::from(&self.rec),
            source: String::from(&self.source),
        }
    }
}

impl std::fmt::Display for PlaylistItemHeader {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.write_str("id:")?;
        fmt.write_str(&self.id)?;
        fmt.write_str(", name:")?;
        fmt.write_str(&self.name)?;
        fmt.write_str(", logo:")?;
        fmt.write_str(&self.logo)?;
        fmt.write_str(", logo-small:")?;
        fmt.write_str(&self.logo_small)?;
        fmt.write_str(", group:")?;
        fmt.write_str(&self.group)?;
        fmt.write_str(", title:")?;
        fmt.write_str(&self.title)?;
        fmt.write_str(", parent-code:")?;
        fmt.write_str(&self.parent_code)?;
        fmt.write_str(", audio-track:")?;
        fmt.write_str(&self.audio_track)?;
        fmt.write_str(", timeshift:")?;
        fmt.write_str(&self.time_shift)?;
        fmt.write_str(", rec:")?;
        fmt.write_str(&self.rec)?;
        fmt.write_str(", source:")?;
        fmt.write_str(&self.source)?;
        Ok(())
    }
}

pub struct PlaylistItem {
    pub header: PlaylistItemHeader,
    pub url: String,
}

impl Clone for PlaylistItem {
    fn clone(&self) -> PlaylistItem {
        PlaylistItem {
            header: self.header.clone(),
            url: String::from(&self.url),
        }
    }
}

impl std::fmt::Display for PlaylistItem {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.write_str("header:")?;
        fmt.write_str(&self.header.to_string())?;
        fmt.write_str(", url:")?;
        fmt.write_str(&self.url)?;
        Ok(())
    }
}

impl PlaylistItem {
    pub(crate) fn to_m3u(&self) -> String {
        let mut line = format!("#EXTINF:-1 tvg-id=\"{}\" tvg-name=\"{}\" tvg-logo=\"{}\" group-title=\"{}\"", self.header.id, self.header.name, self.header.logo, self.header.group);
        if !self.header.logo_small.is_empty() {
            line = format!("{} tvg-logo-small=\"!{}\"", line, self.header.logo_small);
        }
        if !self.header.parent_code.is_empty() {
            line = format!("{} parent-code=\"!{}\"", line, self.header.parent_code);
        }
        if !self.header.audio_track.is_empty() {
            line = format!("{} audio-track=\"!{}\"", line, self.header.audio_track);
        }
        if !self.header.time_shift.is_empty() {
            line = format!("{} timeschift=\"!{}\"", line, self.header.time_shift);
        }
        if !self.header.rec.is_empty() {
            line = format!("{} rec=\"!{}\"", line, self.header.rec);
        }
        line = format!("{},{}\n{}", line, self.header.title, self.url);
        line
    }
}

pub struct PlaylistGroup {
    pub title: String,
    pub channels: Vec<PlaylistItem>,
}

impl std::fmt::Display for PlaylistGroup {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.write_str("title:")?;
        fmt.write_str(&self.title.to_string())?;
        fmt.write_str(", channels:")?;
        fmt.write_str(&self.channels.len().to_string())?;
        Ok(())
    }
}

fn token_value<'a>(it: &'a mut std::str::Chars) -> String {
    let oc = it.next();
    if oc.is_some() && oc.unwrap() == '"' {
        return get_value(it);
    }
    return String::from("");
}

fn get_value<'a>(it: &'a mut std::str::Chars) -> String {
    let mut result: Vec<char> = vec![];
    let mut oc = it.next();
    while oc.is_some() && oc.unwrap() != '"' {
        result.push(oc.unwrap());
        oc = it.next();
    }
    return String::from(result.iter().collect::<String>());
}

fn token_till(it: &mut std::str::Chars, stop_char: char) -> Option<String> {
    let mut result: Vec<char> = vec![];
    loop {
        let c = it.next();
        if !c.is_some() || c.unwrap() == stop_char {
            break;
        }
        if result.len() == 0 && c.unwrap().is_whitespace() {
            continue;
        }
        result.push(c.unwrap())
    }
    if result.len() > 0 { Some(String::from(result.iter().collect::<String>())) } else { None }
}

fn skip_digit(it: &mut std::str::Chars) -> Option<char> {
    let mut c: Option<char>;
    loop {
        c = it.next();
        if !c.is_some() || !(c.unwrap() == '-' || c.unwrap() == '+' || c.unwrap().is_digit(10)) {
            break;
        }
    }
    c
}

fn decode_header(content: &String) -> PlaylistItemHeader {
    let mut plih = PlaylistItemHeader {
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
    };

    let mut it = content.chars();
    let line_token = token_till(&mut it, ':');
    if line_token == Some(String::from("#EXTINF")) {
        let mut c = skip_digit(&mut it);
        loop {
            if !c.is_some() {
                break;
            }
            match c.unwrap() {
                ',' => plih.title = get_value(&mut it),
                _ => {
                    let token = token_till(&mut it, '=');
                    if token != None {
                        let value = token_value(&mut it);
                        match token.unwrap().as_str() {
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
    plih
}

pub(crate) fn decode(lines: &Vec<String>) -> Vec<PlaylistGroup> {
    let mut groups: std::collections::HashMap<String, Vec<PlaylistItem>> = std::collections::HashMap::new();
    let mut sort_order: Vec<String> = vec![];
    let mut header: Option<String> = None;
    let mut group: Option<String> = None;

    for line in lines {
        if line.starts_with("#EXTINF") {
            header = Some(String::from(line));
            continue;
        }
        if line.starts_with("#EXTGRP") {
            group = Some(String::from(&line[8..]));
            continue;
        }
        if line.starts_with("#") {
            continue;
        }
        if header.is_some() {
            let mut item = PlaylistItem { header: decode_header(&header.unwrap()), url: String::from(line) };
            if group.is_some() && item.header.group.is_empty() {
                item.header.group = String::from(group.unwrap());
            }
            let key = String::from(&item.header.group);
            let key2 = String::from(&item.header.group);
            match groups.entry(key) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(vec![item]);
                    sort_order.push(key2);
                }
                std::collections::hash_map::Entry::Occupied(mut e) => { e.get_mut().push(item); }
            }
        }
        header = None;
        group = None;
    }

    let mut result: Vec<PlaylistGroup> = vec![];
    for (key, value) in groups {
        result.push(PlaylistGroup { title: key, channels: value });
    }
    result.sort_by(|f, s| {
        let i1 = sort_order.iter().position(|r| r == &f.title).unwrap();
        let i2 = sort_order.iter().position(|r| r == &s.title).unwrap();
        return i1.cmp(&i2);
    });
    result
}
