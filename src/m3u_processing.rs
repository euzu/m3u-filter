extern crate unidecode;

use std::cell::RefCell;
use std::io::Write;
use std::sync::{Arc};
use std::thread;
use config::ConfigTarget;
use chrono::Datelike;
use unidecode::unidecode;
use crate::{config, Config, utils, valid_property};
use crate::config::{ConfigInput, InputAffix, InputType, ProcessTargets};
use crate::model::SortOrder::{Asc, Desc};
use crate::filter::{get_field_value, MockValueProcessor, set_field_value, ValueProvider};
use crate::m3u::{FieldAccessor, PlaylistGroup, PlaylistItem, PlaylistItemHeader};
use crate::mapping::{Mapping, MappingValueProcessor};
use crate::model::{ItemField, AFFIX_FIELDS, ProcessingOrder, TargetType};
use crate::service::{get_m3u_playlist, get_xtream_playlist};

macro_rules! open_file {
  ($path:expr) => {{
       match std::fs::File::create($path) {
                Ok(file) => file,
                Err(e) => {
                    println!("cant create file: {:?}", $path);
                    return Err(e);
                }
            }
    }};
}

struct KodiStyle {
    year: regex::Regex,
    season: regex::Regex,
    episode: regex::Regex,
    whitespace: regex::Regex,
}

fn check_write(res: std::io::Result<usize>) -> Result<(), std::io::Error> {
    match res {
        Ok(_) => Ok(()),
        Err(_) => Err(std::io::Error::new(std::io::ErrorKind::Other, "Unable to write file")),
    }
}

fn filter_playlist(playlist: &mut [PlaylistGroup], target: &ConfigTarget, verbose: bool) -> Option<Vec<PlaylistGroup>> {
    if verbose { println!("Filtering {} groups", playlist.len()) }
    let mut new_playlist = Vec::new();
    playlist.iter_mut().for_each(|pg| {
        if verbose { println!("Filtering group {} with {} items", pg.title, pg.channels.len()) }
        let mut channels = Vec::new();
        pg.channels.iter_mut().for_each(|pli| {
            if is_valid(pli, target, verbose) {
                channels.push(pli.clone());
            }
        });
        if verbose { println!("Filtered group {} has now {} items", pg.title, channels.len()) }
        if !channels.is_empty() {
            new_playlist.push(PlaylistGroup {
                title: pg.title.clone(),
                channels,
            });
        }
    });
    Some(new_playlist)
}

fn apply_affixes(playlist: &mut [PlaylistGroup], input: &ConfigInput, verbose: bool) {
    if input.suffix.is_some() || input.prefix.is_some() {
        let validate_affix = |a: &Option<InputAffix>| match a {
            Some(affix) => {
                valid_property!(&affix.field.as_str(), AFFIX_FIELDS) && !affix.value.is_empty()
            }
            _ => false
        };

        let apply_prefix = validate_affix(&input.prefix);
        let apply_suffix = validate_affix(&input.suffix);

        if apply_prefix || apply_suffix {
            let get_affix_applied_value = |header: &mut PlaylistItemHeader, affix: &InputAffix, prefix: bool| {
                if let Some(field_value) = header.get_field(affix.field.as_str()) {
                    return if prefix {
                        format!("{}{}", &affix.value, field_value.as_str())
                    } else {
                        format!("{}{}", field_value.as_str(), &affix.value)
                    };
                }
                String::from(&affix.value)
            };

            playlist.iter_mut().for_each(|group| {
                group.channels.iter_mut().for_each(|channel| {
                    if apply_suffix {
                        if let Some(suffix) = &input.suffix {
                            let value = get_affix_applied_value(&mut channel.header.borrow_mut(), suffix, false);
                            if verbose { println!("Applying input suffix:  {}={}", &suffix.field, &value) }
                            channel.header.borrow_mut().set_field(&suffix.field, value.as_str());
                        }
                    }
                    if apply_prefix {
                        if let Some(prefix) = &input.prefix {
                            let value = get_affix_applied_value(&mut channel.header.borrow_mut(), prefix, true);
                            if verbose { println!("Applying input prefix:  {}={}", &prefix.field, &value) }
                            channel.header.borrow_mut().set_field(&prefix.field, value.as_str());
                        }
                    }
                });
            });
        }
    }
}

type ProcessingPipe = Vec<fn(playlist: &mut [PlaylistGroup], target: &ConfigTarget, verbose: bool) -> Option<Vec<PlaylistGroup>>>;

pub(crate) fn write_m3u(playlist: &mut [PlaylistGroup],
                        input: &ConfigInput,
                        target: &ConfigTarget, cfg: &Config,
                        verbose: bool) -> Result<(), std::io::Error> {
    let pipe: ProcessingPipe =
        match &target.processing_order {
            ProcessingOrder::FRM => vec![filter_playlist, rename_playlist, map_playlist],
            ProcessingOrder::FMR => vec![filter_playlist, map_playlist, rename_playlist],
            ProcessingOrder::RFM => vec![rename_playlist, filter_playlist, map_playlist],
            ProcessingOrder::RMF => vec![rename_playlist, map_playlist, filter_playlist],
            ProcessingOrder::MFR => vec![map_playlist, filter_playlist, rename_playlist],
            ProcessingOrder::MRF => vec![map_playlist, rename_playlist, filter_playlist]
        };

    if verbose { println!("Processing order is {}", &target.processing_order) }

    let mut new_playlist = playlist.to_owned();
    for f in pipe {
        let r = f(&mut new_playlist, target, verbose);
        if let Some(v) = r {
            new_playlist = v;
        }
    }

    apply_affixes(&mut new_playlist, input, verbose);

    sort_playlist(target, &mut new_playlist);
    if let Some(output_type) = &target.output {
        if let TargetType::Strm = output_type { return write_strm_playlist(&target, &cfg, &mut new_playlist) }
    }
    write_m3u_playlist(target, cfg, &mut new_playlist)
}

fn write_m3u_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &mut Vec<PlaylistGroup>) -> Result<(), std::io::Error> {
    if let Some(path) = utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&target.filename))) {
        let mut m3u_file = open_file!(&path);
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
    Ok(())
}

fn sanitize_for_filename(text: &str, underscore_whitespace: bool) -> String {
    return text.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .map(|c| if underscore_whitespace { if c.is_whitespace() { '_' } else { c } } else { c })
        .collect::<String>();
}

fn write_strm_playlist(target: &ConfigTarget, cfg: &Config, new_playlist: &mut Vec<PlaylistGroup>) -> Result<(), std::io::Error> {
    let underscore_whitespace = target.options.as_ref().map_or(false, |o| o.underscore_whitespace);
    let cleanup = target.options.as_ref().map_or(false, |o| o.cleanup);
    let kodi_style = target.options.as_ref().map_or(false, |o| o.kodi_style);

    if let Some(path) = utils::get_file_path(&cfg.working_dir, Some(std::path::PathBuf::from(&target.filename))) {
        if cleanup {
            let _ = std::fs::remove_dir_all(&path);
        }
        if let Err(e) = std::fs::create_dir_all(&path) {
            println!("cant create directory: {:?}", &path);
            return Err(e);
        };
        for pg in new_playlist {
            for pli in &pg.channels {
                let dir_path = path.join(sanitize_for_filename(&pli.header.borrow().group, underscore_whitespace));
                if let Err(e) = std::fs::create_dir_all(&dir_path) {
                    println!("cant create directory: {:?}", &path);
                    return Err(e);
                };
                let mut file_name = sanitize_for_filename(&pli.header.borrow().title, underscore_whitespace);
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
                let mut strm_file = open_file!(&file_path);
                match check_write(strm_file.write(pli.url.as_bytes())) {
                    Ok(_) => (),
                    Err(e) => return Err(e),
                }
            }
        }
    }
    Ok(())
}

fn kodi_style_rename_year(name: &String, style: &KodiStyle) -> (String, Option<String>) {
    let current_date = chrono::Utc::now();
    let cur_year = current_date.year();
    match style.year.find(name) {
        Some(m) => {
            let s_year = &name[m.start()..m.end()];
            let t_year: i32 = s_year.parse().unwrap();
            if t_year > 1900 && t_year <= cur_year {
                let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
                return (new_name, Some(String::from(s_year)));
            }
            (String::from(name), Some(cur_year.to_string()))
        }
        _ => (String::from(name), Some(cur_year.to_string())),
    }
}

fn kodi_style_rename_season(name: &String, style: &KodiStyle) -> (String, Option<String>) {
    match style.season.find(name) {
        Some(m) => {
            let s_season = &name[m.start()..m.end()];
            let season = Some(String::from(&s_season[1..]));
            let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
            (new_name, season)
        }
        _ => (String::from(name), Some(String::from("01"))),
    }
}

fn kodi_style_rename_episode(name: &String, style: &KodiStyle) -> (String, Option<String>) {
    match style.episode.find(name) {
        Some(m) => {
            let s_episode = &name[m.start()..m.end()];
            let episode = Some(String::from(&s_episode[1..]));
            let new_name = format!("{}{}", &name[0..m.start()], &name[m.end()..]);
            (new_name, episode)
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
    String::from(name)
}

fn sort_playlist(target: &ConfigTarget, new_playlist: &mut [PlaylistGroup]) {
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


fn is_valid(pli: &mut PlaylistItem, target: &ConfigTarget, verbose: bool) -> bool {
    let provider = ValueProvider { pli: RefCell::new(pli) };
    target.filter(&provider, verbose)
}

fn exec_rename(pli: &mut PlaylistItem, rename: &Option<Vec<config::ConfigRename>>, verbose: bool) {
    if let Some(renames) = rename {
        if !renames.is_empty() {
            let result = pli;
            for r in renames {
                let value = get_field_value(result, &r.field);
                let cap = r.re.as_ref().unwrap().replace_all(value.as_str(), &r.new_name);
                if verbose { println!("Renamed {}={} to {}", &r.field, value, cap) }
                let value = cap.into_owned();
                set_field_value(result, &r.field, value);
            }
        }
    }
}

fn rename_playlist(playlist: &mut [PlaylistGroup], target: &ConfigTarget, verbose: bool) -> Option<Vec<PlaylistGroup>> {
    match &target.rename {
        Some(renames) => {
            if !renames.is_empty() {
                let mut new_playlist: Vec<PlaylistGroup> = Vec::new();
                for g in playlist {
                    let mut grp = g.clone();
                    for r in renames {
                        if let ItemField::Group = r.field {
                            let cap = r.re.as_ref().unwrap().replace_all(&grp.title, &r.new_name);
                            if verbose { println!("Renamed group {} to {}", &grp.title, cap); }
                            grp.title = cap.into_owned();
                        }
                    }

                    grp.channels.iter_mut().for_each(|pli| exec_rename(pli, &target.rename, verbose));
                    new_playlist.push(grp);
                }
                return Some(new_playlist);
            }
            None
        }
        _ => None
    }
}

macro_rules! apply_pattern {
    ($pattern:expr, $provider:expr, $processor:expr, $verbose:expr) => {{
            match $pattern {
                Some(ptrn) => {
                    ptrn.filter($provider, $processor, $verbose);
                },
                _ => {}
            };
    }};
}

fn map_channel(channel: &PlaylistItem, mapping: &Mapping, verbose: bool) -> PlaylistItem {
    if !mapping.mapper.is_empty() {
        let header = channel.header.borrow();
        let channel_name = if mapping.match_as_ascii { unidecode(&header.name) } else { String::from(&header.name) };
        if verbose && mapping.match_as_ascii { println!("Decoded {} for matching to {}", &header.name, &channel_name) };
        drop(header);
        let ref_chan = RefCell::new(channel);
        let provider = ValueProvider { pli:  ref_chan.clone()};
        let mut mock_processor = MockValueProcessor {};

        for m in &mapping.mapper {
            let mut processor = MappingValueProcessor { pli: ref_chan.clone(), mapper: RefCell::new(m) };
            match &m._filter {
                Some(filter) => {
                    if filter.filter(&provider, &mut mock_processor, verbose) {
                        apply_pattern!(&m._pattern, &provider, &mut processor, verbose);
                    }
                }
                _ => {
                    apply_pattern!(&m._pattern, &provider, &mut processor, verbose);
                }
            };
        }
    }
    channel.clone()
}

fn map_playlist(playlist: &mut [PlaylistGroup], target: &ConfigTarget, verbose: bool) -> Option<Vec<PlaylistGroup>> {
    if verbose { println!("Mapping") }
    if target._mapping.is_some() {
        let new_playlist: Vec<PlaylistGroup> = playlist.iter().map(|playlist_group| {
            let mut grp = playlist_group.clone();
            let mappings = target._mapping.as_ref().unwrap();
            mappings.iter().filter(|mapping| !mapping.mapper.is_empty()).for_each(|mapping|
                   grp.channels = grp.channels.iter_mut().map(|chan| map_channel(chan, mapping, verbose)).collect());
            grp
        }).collect();

        // if the group names are changed, restructure channels to the right groups
        // we use
        let mut new_groups: Vec<PlaylistGroup> = Vec::new();
        for playlist_group in new_playlist {
            for channel in &playlist_group.channels {
                let title = &channel.header.borrow().group;
                match new_groups.iter_mut().find(|x| &*x.title == title) {
                    Some(grp) => grp.channels.push(channel.clone()),
                    _ => new_groups.push(PlaylistGroup { title: String::from(title), channels: vec![channel.clone()] })
                }
            }
        }
        Some(new_groups)
    } else {
        None
    }
}

fn process_source(cfg: Arc<Config>, source_idx: usize, user_targets: Arc<ProcessTargets>, verbose: bool) {
    let source = cfg.sources.get(source_idx).unwrap();
    let input = &source.input;
    if input.enabled || (user_targets.enabled && user_targets.has_input(input.id)) {
        let mut result = match input.input_type {
            InputType::M3u => get_m3u_playlist(input, &cfg.working_dir, verbose),
            InputType::Xtream => get_xtream_playlist(input, &cfg.working_dir, verbose),
        };
        if let Some(playlist) = result.as_mut() {
            if playlist.is_empty() {
                if verbose { println!("Input file is empty") }
            } else {
                if verbose { println!("Input file has {} groups", playlist.len()) }
                source.targets.iter().for_each(|target| {
                    let should_process = (!user_targets.enabled && target.enabled)
                        || (user_targets.enabled && user_targets.has_target(target.id));
                    if should_process {
                        match write_m3u(playlist, input, target, &cfg, verbose) {
                            Ok(_) => (),
                            Err(e) => println!("Failed to write file: {}", e)
                        }
                    }
                });
            }
        }
    }
}

pub fn process_sources(cfg: Arc<Config>, user_targets: &ProcessTargets, verbose: bool) {
    let mut handle_list = vec![];
    let thread_num = cfg.threads;
    let process_parallel = thread_num > 1 && cfg.sources.len() > 1;
    if verbose && process_parallel { println!("Using {} threads", thread_num) }

    for (index, _) in cfg.sources.iter().enumerate() {
        let config = cfg.clone();
        let usr_targets = Arc::new(user_targets.clone());
        let process = move || process_source(config, index, usr_targets, verbose);
        if process_parallel {
            let handles = &mut handle_list;
            handles.push(thread::spawn(process));
            if handles.len() as u8 >= thread_num {
                while let Some(handle) = handles.pop() {
                    let _ = handle.join();
                }
            }
        } else {
            process();
        }
    }
    for handle in handle_list {
        let _ = handle.join();
    }
}
