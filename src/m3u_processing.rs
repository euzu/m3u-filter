extern crate unidecode;
use std::io::Write;
use config::ConfigTarget;
use chrono::Datelike;
use unidecode::unidecode;
use crate::{config, Config, get_playlist, m3u, utils};
use crate::model::SortOrder::{Asc, Desc};
use crate::filter::ValueProvider;
use crate::m3u::{PlaylistGroup, PlaylistItem};
use crate::mapping::{Mapping, MappingTag};
use crate::model::{ItemField, ProcessingOrder, TargetType};


struct KodiStyle {
    year: regex::Regex,
    season: regex::Regex,
    episode: regex::Regex,
    whitespace: regex::Regex,
}

fn check_write(res: std::io::Result<usize>) -> Result<(), std::io::Error> {
    match res {
        Ok(_) => Ok(()),
        Err(_) => return Err(std::io::Error::new(std::io::ErrorKind::Other, "Unable to write file")),
    }
}

fn filter_playlist(playlist: &Vec<PlaylistGroup>, target: &ConfigTarget, verbose: bool) -> Option<Vec<PlaylistGroup>> {
    if verbose { println!("Filtering {} groups", playlist.len()) }
    let mut new_playlist = Vec::new();
    for pg in playlist {
        if verbose { println!("Filtering group {} with {} items", pg.title, pg.channels.len()) }
        let mut channels = Vec::new();
        for pli in &pg.channels {
            if is_valid(&pli, &target) {
                channels.push(pli.clone());
            }
        }
        if verbose { println!("Filtered group {} has now {} items", pg.title, channels.len()) }
        if channels.len() > 0 {
            new_playlist.push(PlaylistGroup {
                title: pg.title.clone(),
                channels,
            });
        }
    }
    Some(new_playlist)
}

pub(crate) fn write_m3u(playlist: &Vec<m3u::PlaylistGroup>, 
                        target: &config::ConfigTarget, cfg: &config::Config, 
                        verbose: bool) -> Result<(), std::io::Error> {
    let pipe : Vec<fn(playlist: &Vec<PlaylistGroup>, target: &ConfigTarget, verbose: bool) -> Option<Vec<PlaylistGroup>>> = match &target.processing_order {
        ProcessingOrder::Frm => vec![filter_playlist, rename_playlist, map_playlist],
        ProcessingOrder::Fmr => vec![filter_playlist, map_playlist, rename_playlist],
        ProcessingOrder::Rfm => vec![rename_playlist, filter_playlist, map_playlist],
        ProcessingOrder::Rmf => vec![rename_playlist, map_playlist, filter_playlist],
        ProcessingOrder::Mfr => vec![map_playlist, filter_playlist, rename_playlist],
        ProcessingOrder::Mrf => vec![map_playlist, rename_playlist, filter_playlist]
    };

    if verbose { println!("Processing order is {}", &target.processing_order)}
    
    let mut new_playlist= playlist.clone();
    for f in pipe {
        let r =  f(&new_playlist, &target, verbose);
        if r.is_some() {
            new_playlist = r.unwrap();
        }
    }

    sort_playlist(target, &mut new_playlist);
    match &target.output {
        Some(output_type) => {
            match output_type {
                TargetType::Strm => return write_strm_playlist(&target, &cfg, &mut new_playlist),
                _ => {}
            }
        }
        _ => {}
    }
    return write_m3u_playlist(&target, &cfg, &mut new_playlist);
}

fn write_m3u_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &mut Vec<PlaylistGroup>) -> Result<(), std::io::Error> {
    match utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&target.filename))) {
        Some(path) => {
            let mut m3u_file = match std::fs::File::create(&path) {
                Ok(file) => file,
                Err(e) => {
                    println!("cant create file: {:?}", &path);
                    return Err(e);
                }
            };

            match check_write(m3u_file.write(b"#EXTM3U\n")) {
                Ok(_) => (),
                Err(e) => return Err(e),
            }
            for pg in new_playlist {
                for pli in &pg.channels {
                    let content = pli.to_m3u(&target.options);
                    match check_write(m3u_file.write(content.as_bytes())) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }
                    match check_write(m3u_file.write(b"\n")) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }
                }
            }
        }
        None => (),
    }
    Ok(())
}

fn sanitize_for_filename(text: &String, underscore_whitespace: bool) -> String {
    return text.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .map(|c| if underscore_whitespace { if c.is_whitespace() { '_' } else { c } } else { c })
        .collect::<String>();
}

fn write_strm_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &mut Vec<PlaylistGroup>) -> Result<(), std::io::Error> {
    let underscore_whitespace = target.options.as_ref().map_or(false, |o| o.underscore_whitespace);
    let cleanup = target.options.as_ref().map_or(false, |o| o.cleanup);
    let kodi_style = target.options.as_ref().map_or(false, |o| o.kodi_style);

    match utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&target.filename))) {
        Some(path) => {
            if cleanup {
                let _ = std::fs::remove_dir_all(&path);
            }
            match std::fs::create_dir_all(&path) {
                Err(e) => {
                    println!("cant create directory: {:?}", &path);
                    return Err(e);
                }
                _ => {}
            };
            for pg in new_playlist {
                for pli in &pg.channels {
                    let dir_path = path.join(sanitize_for_filename(&pli.header.group, underscore_whitespace));
                    match std::fs::create_dir_all(&dir_path) {
                        Err(e) => {
                            println!("cant create directory: {:?}", &path);
                            return Err(e);
                        }
                        _ => {}
                    };
                    let mut file_name = sanitize_for_filename(&pli.header.title, underscore_whitespace);
                    if kodi_style {
                        let style = KodiStyle {
                            season: regex::Regex::new(r"[Ss]\d\d").unwrap(),
                            episode: regex::Regex::new(r"[Ee]\d\d").unwrap(),
                            year: regex::Regex::new(r"\d\d\d\d").unwrap(),
                            whitespace: regex::Regex::new(r"\s+").unwrap(),
                        };
                        file_name = kodi_style_rename(&file_name, &style);
                    }
                    let file_path = dir_path.join(format!("{}.strm", file_name));
                    let mut strm_file = match std::fs::File::create(&file_path) {
                        Ok(file) => file,
                        Err(e) => {
                            println!("cant create file: {:?}", &file_path);
                            return Err(e);
                        }
                    };
                    match check_write(strm_file.write(pli.url.as_bytes())) {
                        Ok(_) => (),
                        Err(e) => return Err(e),
                    }
                }
            }
        }
        None => (),
    }
    Ok(())
}

fn kodi_style_rename_year(name: &String, style: &KodiStyle) -> (String, Option<String>) {
    let current_date = chrono::Utc::now();
    let cur_year = current_date.year();
    match style.year.find(&name) {
        Some(m) => {
            let s_year = &name[m.start()..m.end()];
            let t_year: i32 = s_year.parse().unwrap();
            if t_year > 1900 && t_year <= cur_year {
                let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
                return (new_name, Some(String::from(s_year)));
            }
            return (String::from(name), Some(cur_year.to_string()));
        }
        _ => (String::from(name), Some(cur_year.to_string())),
    }
}

fn kodi_style_rename_season(name: &String, style: &KodiStyle) -> (String, Option<String>) {
    match style.season.find(&name) {
        Some(m) => {
            let s_season = &name[m.start()..m.end()];
            let season = Some(String::from(&s_season[1..]));
            let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
            return (new_name, season);
        }
        _ => (String::from(name), Some(String::from("01"))),
    }
}

fn kodi_style_rename_episode(name: &String, style: &KodiStyle) -> (String, Option<String>) {
    match style.episode.find(&name) {
        Some(m) => {
            let s_episode = &name[m.start()..m.end()];
            let episode = Some(String::from(&s_episode[1..]));
            let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
            return (new_name, episode);
        }
        _ => (String::from(name), None),
    }
}

fn kodi_style_rename(name: &String, style: &KodiStyle) -> String {
    let (work_name_1, year) = kodi_style_rename_year(name, style);
    let (work_name_2, season) = kodi_style_rename_season(&work_name_1, style);
    let (work_name_3, episode) = kodi_style_rename_episode(&work_name_2, style);
    if year.is_some() && season.is_some() && episode.is_some() {
        let formatted = format!("{} ({}) S{}E{}", work_name_3, year.unwrap(), season.unwrap(), episode.unwrap());
        return String::from(style.whitespace.replace_all(formatted.as_str(), " ").as_ref());
    }
    return String::from(name);
}

fn sort_playlist(target: &ConfigTarget, new_playlist: &mut Vec<PlaylistGroup>) {
    if let Some(sort) = &target.sort {
        new_playlist.sort_by(|a, b| {
            let ordering = a.title.partial_cmp(&b.title).unwrap();
            match sort.order {
                Asc => ordering,
                Desc => ordering.reverse()
            }
        });
    }
}


fn is_valid(pli: &m3u::PlaylistItem, target: &ConfigTarget) -> bool {
    let provider = ValueProvider { pli };
    return target.filter(&provider);
}

fn exec_rename(pli: &mut m3u::PlaylistItem, rename: &Option<Vec<config::ConfigRename>>, verbose: bool) {
    match rename {
        Some(renames) => {
            if renames.len() > 0 {
                let mut result = pli;
                for r in renames {
                    let value = get_field_value(&result, &r.field);
                    let cap = r.re.as_ref().unwrap().replace_all(value, &r.new_name);
                    if verbose { println!("Renamed {}={} to {}", &r.field, value, cap)}
                    let value = cap.into_owned();
                    set_field_value(&mut result, &r.field, value);
                }
            }
        }
        _ => {}
    }
}

fn rename_playlist(playlist: &Vec<PlaylistGroup>, target: &ConfigTarget, verbose: bool) -> Option<Vec<PlaylistGroup>> {
    match &target.rename {
        Some(renames) => {
            if renames.len() > 0 {
                let mut new_playlist: Vec<m3u::PlaylistGroup> = Vec::new();
                for g in playlist {
                    let mut grp = g.clone();
                    for r in renames {
                        match r.field {
                            ItemField::Group => {
                                let cap = r.re.as_ref().unwrap().replace_all(&grp.title, &r.new_name);
                                if verbose { println!("Renamed group {} to {}", &grp.title, cap); }
                                grp.title = cap.into_owned();
                            }
                            _ => {}
                        }
                    }

                    for pli in &mut grp.channels {
                        exec_rename(pli, &target.rename, verbose)
                    }
                    new_playlist.push(grp);
                }
                return Some(new_playlist)
            }
            None
        }
        _ => None
    }
}

fn get_mapping_tag(mapping: &&Mapping) -> MappingTag {
    let mapping_tag = match &mapping.tag {
        Some(mtag) => mtag.clone(),
        _ => MappingTag {
            captures: vec![],
            concat: "".to_string(),
            prefix: "".to_string(),
            suffix: "".to_string(),
        }
    };
    mapping_tag
}

fn map_channel(channel: &PlaylistItem, mapping: &Mapping, verbose: bool) -> PlaylistItem {
    if mapping.mapper.len() > 0 {
        let channel_name = if mapping.match_as_ascii { unidecode(&channel.header.name) } else { String::from(&channel.header.name) };
        if verbose && mapping.match_as_ascii { println!("Decoded {} for matching to {}", &channel.header.name, &channel_name)};
        let mut tag = "".to_string();
        let mapping_tag = get_mapping_tag(&mapping);

        for m in &mapping.mapper {
            match &m._re {
                Some(regexps) => {
                    for re in regexps {
                        if re.re.is_match(&channel_name) {
                            let mut chan = channel.clone();
                            let mut chan_name = m.tvg_name.clone();
                            if re.captures.len() > 0 {
                                let captures_opt = re.re.captures(&channel_name);
                                if captures_opt.is_some() {
                                    let captures = captures_opt.unwrap();
                                    for cname in &re.captures {
                                        let match_opt = captures.name(cname.as_str());
                                        let repl = if match_opt.is_some() {
                                            match_opt.map_or("", |m| m.as_str())
                                        } else {
                                            ""
                                        };
                                        chan_name = String::from(chan_name.replace(format!("${}", cname.as_str()).as_str(), repl));

                                        for c in &mapping_tag.captures {
                                            if c.eq(cname) {
                                                if tag.is_empty() {
                                                    tag = String::from(repl);
                                                } else {
                                                    tag = format!("{}{}{}", tag,  &mapping_tag.concat, String::from(repl))
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            if !tag.is_empty() {
                                tag = format!("{}{}{}", mapping_tag.prefix, tag, mapping_tag.suffix)
                            }

                            let new_name = format!("{}{}", &chan_name, tag);
                            if verbose { println!("Mapped {} to {}", &channel.header.name, new_name)}
                            chan.header.name = new_name;
                            chan.header.chno = m.tvg_chno.clone();
                            chan.header.id = m.tvg_id.clone();
                            chan.header.logo = m.tvg_logo.clone();
                            let mut split: Vec<String> = channel.header.group.split("|").map(|s| String::from(s.trim())).collect();
                            split.append(m.group_title.clone().as_mut());
                            chan.header.group = split.join("|");
                            return chan;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    return channel.clone();
}

fn map_playlist(playlist: &Vec<PlaylistGroup>, target: &ConfigTarget, verbose: bool) -> Option<Vec<PlaylistGroup>> {
    if target._mapping.is_some() {
        let mut new_playlist: Vec<m3u::PlaylistGroup> = Vec::new();
        for g in playlist {
            let mut grp = g.clone();
            let mappings = target._mapping.as_ref().unwrap();
            for mapping in mappings {
                if mapping.mapper.len() > 0 {
                    grp.channels = grp.channels.iter().map(|chan| map_channel(&chan, &mapping, verbose)).collect();
                }
            }
            new_playlist.push(grp);
        }
        Some(new_playlist)
    } else {
        None
    }
}

fn get_field_value<'a>(pli: &'a m3u::PlaylistItem, field: &ItemField) -> &'a str {
    let value = match field {
        ItemField::Group => pli.header.group.as_str(),
        ItemField::Name => pli.header.name.as_str(),
        ItemField::Title => pli.header.title.as_str(),
        ItemField::Url => pli.url.as_str(),
    };
    value
}

fn set_field_value(pli: &mut m3u::PlaylistItem, field: &ItemField, value: String) -> () {
    let header = &mut pli.header;
    match field {
        ItemField::Group => header.group = value,
        ItemField::Name => header.name = value,
        ItemField::Title => header.title = value,
        ItemField::Url => {}
    };
}

pub fn process_targets(cfg: &Config, verbose: bool) {
    for source in cfg.sources.iter() {
        let url_str = source.input.url.as_str();
        let persist_file: Option<std::path::PathBuf> =
            if source.input.persist.is_empty() { None } else { utils::prepare_persist_path(source.input.persist.as_str()) };
        let file_path = utils::get_file_path(&cfg.working_dir, persist_file);
        if verbose { println!("persist file: {:?}", &file_path); }

        let result = get_playlist(&cfg.working_dir, url_str, file_path);
        match &result {
            Some(playlist) => {
                for target in source.targets.iter() {
                    match write_m3u(playlist, target, &cfg, verbose) {
                        Ok(_) => (),
                        Err(e) => println!("Failed to write file: {}", e)
                    }
                }
            }
            None => ()
        }
    }
}
