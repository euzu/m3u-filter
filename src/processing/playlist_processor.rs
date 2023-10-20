extern crate unidecode;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

use log::{debug, error, info};
use unidecode::unidecode;

use crate::{Config, create_m3u_filter_error_result, get_errors_notify_message, model::config, valid_property};
use crate::download::{get_m3u_playlist, get_xtream_playlist};
use crate::filter::{get_field_value, MockValueProcessor, set_field_value, ValueProvider};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::messaging::{MsgKind, send_message};
use crate::model::config::{ConfigTarget, default_as_default, InputAffix, InputType, ProcessTargets};
use crate::model::mapping::{Mapping, MappingValueProcessor};
use crate::model::model_config::{AFFIX_FIELDS, ItemField, ProcessingOrder, SortOrder::{Asc, Desc}};
use crate::model::model_m3u::{FetchedPlaylist, FieldAccessor, PlaylistGroup, PlaylistItem, PlaylistItemHeader};
use crate::model::stats::{InputStats, PlaylistStats};
use crate::processing::playlist_watch::process_group_watch;
use crate::repository::m3u_repository::write_playlist;
use crate::repository::xtream_repository::xtream_save_playlist;

fn filter_playlist(playlist: &mut [PlaylistGroup], target: &ConfigTarget) -> Option<Vec<PlaylistGroup>> {
    debug!("Filtering {} groups", playlist.len());
    let mut new_playlist = Vec::new();
    playlist.iter_mut().for_each(|pg| {
        debug!("Filtering group {} with {} items", pg.title, pg.channels.len());
        let mut channels = Vec::new();
        pg.channels.iter_mut().for_each(|pli| {
            if is_valid(pli, target) {
                channels.push(pli.clone());
            }
        });
        debug!("Filtered group {} has now {} items", pg.title, channels.len());
        if !channels.is_empty() {
            new_playlist.push(PlaylistGroup {
                id: pg.id,
                title: pg.title.clone(),
                channels,
                xtream_cluster: pg.xtream_cluster.clone(),
            });
        }
    });
    Some(new_playlist)
}

fn apply_affixes(fetched_playlists: &mut [FetchedPlaylist]) {
    fetched_playlists.iter_mut().for_each(|fetched_playlist| {
        let FetchedPlaylist { input, playlist } = fetched_playlist;
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
                                debug!("Applying input suffix:  {}={}", &suffix.field, &value);
                                channel.header.borrow_mut().set_field(&suffix.field, value.as_str());
                            }
                        }
                        if apply_prefix {
                            if let Some(prefix) = &input.prefix {
                                let value = get_affix_applied_value(&mut channel.header.borrow_mut(), prefix, true);
                                debug!("Applying input prefix:  {}={}", &prefix.field, &value);
                                channel.header.borrow_mut().set_field(&prefix.field, value.as_str());
                            }
                        }
                    });
                });
            }
        }
    });
}

fn sort_playlist(target: &ConfigTarget, new_playlist: &mut [PlaylistGroup]) {
    if let Some(sort) = &target.sort {
        let match_as_ascii = &sort.match_as_ascii;
        if let Some(group_sort) = &sort.groups {
            new_playlist.sort_by(|a, b| {
                let value_a = if *match_as_ascii { unidecode(&a.title) } else { String::from(&a.title) };
                let value_b = if *match_as_ascii { unidecode(&b.title) } else { String::from(&b.title) };
                let ordering = value_a.partial_cmp(&value_b).unwrap();
                match group_sort.order {
                    Asc => ordering,
                    Desc => ordering.reverse()
                }
            });
        }
        if let Some(channel_sorts) = &sort.channels {
            channel_sorts.iter().for_each(|channel_sort| {
                let regexp = channel_sort.re.as_ref().unwrap();
                new_playlist.iter_mut().for_each(|group| {
                    let group_title = if *match_as_ascii { unidecode(&group.title) } else { String::from(&group.title) };
                    let is_match = regexp.is_match(group_title.as_str());
                    if is_match {
                        group.channels.sort_by(|a, b| {
                            let raw_value_a = get_field_value(a, &channel_sort.field);
                            let raw_value_b = get_field_value(b, &channel_sort.field);
                            let value_a = if *match_as_ascii { unidecode(&raw_value_a) } else { raw_value_a };
                            let value_b = if *match_as_ascii { unidecode(&raw_value_b) } else { raw_value_b };
                            let ordering = value_a.partial_cmp(&value_b).unwrap();
                            match channel_sort.order {
                                Asc => ordering,
                                Desc => ordering.reverse()
                            }
                        });
                    }
                });
            });
        }
    }
}


fn is_valid(pli: &mut PlaylistItem, target: &ConfigTarget) -> bool {
    let provider = ValueProvider { pli: RefCell::new(pli) };
    target.filter(&provider)
}

fn exec_rename(pli: &mut PlaylistItem, rename: &Option<Vec<config::ConfigRename>>) {
    if let Some(renames) = rename {
        if !renames.is_empty() {
            let result = pli;
            for r in renames {
                let value = get_field_value(result, &r.field);
                let cap = r.re.as_ref().unwrap().replace_all(value.as_str(), &r.new_name);
                debug!("Renamed {}={} to {}", &r.field, value, cap);
                let value = cap.into_owned();
                set_field_value(result, &r.field, value);
            }
        }
    }
}

fn rename_playlist(playlist: &mut [PlaylistGroup], target: &ConfigTarget) -> Option<Vec<PlaylistGroup>> {
    match &target.rename {
        Some(renames) => {
            if !renames.is_empty() {
                let mut new_playlist: Vec<PlaylistGroup> = Vec::new();
                for g in playlist {
                    let mut grp = g.clone();
                    for r in renames {
                        if let ItemField::Group = r.field {
                            let cap = r.re.as_ref().unwrap().replace_all(&grp.title, &r.new_name);
                            debug!("Renamed group {} to {} for {}", &grp.title, cap, target.name);
                            grp.title = cap.into_owned();
                        }
                    }

                    grp.channels.iter_mut().for_each(|pli| exec_rename(pli, &target.rename));
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
    ($pattern:expr, $provider:expr, $processor:expr) => {{
            match $pattern {
                Some(ptrn) => {
                    ptrn.filter($provider, $processor);
                },
                _ => {}
            };
    }};
}

fn map_channel(channel: &PlaylistItem, mapping: &Mapping) -> PlaylistItem {
    if !mapping.mapper.is_empty() {
        let header = channel.header.borrow();
        let channel_name = if mapping.match_as_ascii { unidecode(&header.name) } else { String::from(&header.name) };
        if mapping.match_as_ascii { debug!("Decoded {} for matching to {}", &header.name, &channel_name); };
        drop(header);
        let ref_chan = RefCell::new(channel);
        let provider = ValueProvider { pli: ref_chan.clone() };
        let mut mock_processor = MockValueProcessor {};

        for m in &mapping.mapper {
            let mut processor = MappingValueProcessor { pli: ref_chan.clone(), mapper: RefCell::new(m) };
            match &m._filter {
                Some(filter) => {
                    if filter.filter(&provider, &mut mock_processor) {
                        apply_pattern!(&m._pattern, &provider, &mut processor);
                    }
                }
                _ => {
                    apply_pattern!(&m._pattern, &provider, &mut processor);
                }
            };
        }
    }
    channel.clone()
}

fn map_playlist(playlist: &mut [PlaylistGroup], target: &ConfigTarget) -> Option<Vec<PlaylistGroup>> {
    if target._mapping.is_some() {
        let new_playlist: Vec<PlaylistGroup> = playlist.iter().map(|playlist_group| {
            let mut grp = playlist_group.clone();
            let mappings = target._mapping.as_ref().unwrap();
            mappings.iter().filter(|mapping| !mapping.mapper.is_empty()).for_each(|mapping|
                grp.channels = grp.channels.iter_mut().map(|chan| map_channel(chan, mapping)).collect());
            grp
        }).collect();

        // if the group names are changed, restructure channels to the right groups
        // we use
        let mut new_groups: Vec<PlaylistGroup> = Vec::new();
        let mut grp_id: i32 = 0;
        for playlist_group in new_playlist {
            for channel in &playlist_group.channels {
                let cluster = &channel.header.borrow().xtream_cluster;
                let title = &channel.header.borrow().group;
                match new_groups.iter_mut().find(|x| &*x.title == title) {
                    Some(grp) => grp.channels.push(channel.clone()),
                    _ => {
                        grp_id += 1;
                        new_groups.push(PlaylistGroup {
                            id: grp_id,
                            title: String::from(title),
                            channels: vec![channel.clone()],
                            xtream_cluster: cluster.clone(),
                        })
                    }
                }
            }
        }
        Some(new_groups)
    } else {
        None
    }
}

// If no input is enabled but the user set the target as command line argument,
// we force the input to be enabled.
// If there are enabled input, then only these are used.
fn is_input_enabled(enabled_inputs: usize, input_enabled: bool, input_id: u16, user_targets: &ProcessTargets) -> bool {
    if enabled_inputs == 0 {
        return user_targets.enabled && user_targets.has_input(input_id);
    }
    input_enabled
}

fn process_source(cfg: Arc<Config>, source_idx: usize, user_targets: Arc<ProcessTargets>) -> (Vec<InputStats>, Vec<M3uFilterError>) {
    let source = cfg.sources.get(source_idx).unwrap();
    let mut all_playlist = Vec::new();
    let enabled_inputs = source.inputs.iter().filter(|item| item.enabled).count();
    let mut errors = vec![];
    let mut stats = HashMap::<u16, InputStats>::new();
    for input in &source.inputs {
        //if input.enabled || (user_targets.enabled && user_targets.has_input(input.id)) {
        let input_id = input.id;
        if is_input_enabled(enabled_inputs, input.enabled, input_id, &user_targets) {
            let (playlist, mut error_list) = match input.input_type {
                InputType::M3u => get_m3u_playlist(&cfg, input, &cfg.working_dir),
                InputType::Xtream => get_xtream_playlist(input, &cfg.working_dir),
            };
            error_list.drain(..).for_each(|err| errors.push(err));
            let input_name = match &input.name {
                None => input.url.as_str(),
                Some(name_val) => name_val.as_str()
            };
            let group_count = playlist.len();
            let channel_count = playlist.iter()
                .map(|group| group.channels.len())
                .sum();
            if playlist.is_empty() {
                info!("source is empty {}", input.url);
                errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("source is empty {}", input_name)));
            } else {
                all_playlist.push(
                    FetchedPlaylist {
                        input,
                        playlist,
                    }
                );
            }
            stats.insert(input_id, InputStats {
                name: input_name.to_string(),
                input_type: input.input_type.clone(),
                error_count: error_list.len(),
                raw_stats: PlaylistStats {
                    group_count,
                    channel_count,
                },
                processed_stats: PlaylistStats {
                    group_count: 0,
                    channel_count: 0,
                },
            });
        }
    }
    if all_playlist.is_empty() {
        debug!("Source at {} input is empty", source_idx);
        errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Source at {} input is empty", source_idx)));
    } else {
        debug!("Input has {} groups", all_playlist.len());
        source.targets.iter().for_each(|target| {
            let should_process = (!user_targets.enabled && target.enabled)
                || (user_targets.enabled && user_targets.has_target(target.id));
            if should_process {
                match process_playlist(&mut all_playlist, target, &cfg, &mut stats) {
                    Ok(_) => {}
                    Err(err) => errors.push(err)
                }
            }
        });
    }
    (stats.drain().map(|(_, v)| v).collect(), errors)
}

pub(crate) fn process_sources(config: Arc<Config>, user_targets: Arc<ProcessTargets>) -> (Vec<InputStats>, Vec<M3uFilterError>) {
    let mut handle_list = vec![];
    let thread_num = config.threads;
    let process_parallel = thread_num > 1 && config.sources.len() > 1;
    if process_parallel { debug!("Using {} threads", thread_num); }

    let errors = Arc::new(Mutex::<Vec<M3uFilterError>>::new(vec![]));
    let stats = Arc::new(Mutex::<Vec<InputStats>>::new(vec![]));
    for (index, _) in config.sources.iter().enumerate() {
        let shared_errors = errors.clone();
        let shared_stats = stats.clone();
        let cfg = config.clone();
        let usr_trgts = user_targets.clone();
        let process = move || {
            let (mut res_stats, mut res_errors) = process_source(cfg, index, usr_trgts);
            res_errors.drain(..)
                .for_each(|err| shared_errors.lock().unwrap().push(err));
            res_stats.drain(..)
                .for_each(|stat| shared_stats.lock().unwrap().push(stat));
        };
        if process_parallel {
            let handles = &mut handle_list;
            handles.push(thread::spawn(process));
            if handles.len() as u8 >= thread_num {
                handles.drain(..).for_each(|handle| { let _ = handle.join(); });
            }
        } else {
            process();
        }
    }
    for handle in handle_list {
        let _ = handle.join();
    }
    (Arc::try_unwrap(stats).unwrap().into_inner().unwrap(), Arc::try_unwrap(errors).unwrap().into_inner().unwrap())
}


type ProcessingPipe = Vec<fn(playlist: &mut [PlaylistGroup], target: &ConfigTarget) -> Option<Vec<PlaylistGroup>>>;

fn get_processing_pipe(target: &ConfigTarget) -> ProcessingPipe {
    match &target.processing_order {
        ProcessingOrder::Frm => vec![filter_playlist, rename_playlist, map_playlist],
        ProcessingOrder::Fmr => vec![filter_playlist, map_playlist, rename_playlist],
        ProcessingOrder::Rfm => vec![rename_playlist, filter_playlist, map_playlist],
        ProcessingOrder::Rmf => vec![rename_playlist, map_playlist, filter_playlist],
        ProcessingOrder::Mfr => vec![map_playlist, filter_playlist, rename_playlist],
        ProcessingOrder::Mrf => vec![map_playlist, rename_playlist, filter_playlist]
    }
}

pub(crate) fn process_playlist(playlists: &mut [FetchedPlaylist],
                               target: &ConfigTarget, cfg: &Config,
                               stats: &mut HashMap<u16, InputStats>) -> Result<(), M3uFilterError> {
    let pipe = get_processing_pipe(target);

    debug!("Processing order is {}", &target.processing_order);

    let mut new_fetched_playlists: Vec<FetchedPlaylist> = vec![];
    playlists.iter_mut().for_each(|fpl| {
        let mut new_fpl = FetchedPlaylist {
            input: fpl.input,
            playlist: fpl.playlist.clone(),
        };
        for f in &pipe {
            let playlist = &mut new_fpl.playlist;
            let r = f(playlist, target);
            if let Some(v) = r {
                new_fpl.playlist = v;
            }
        }
        // stats
        let input_stats = stats.get_mut(&new_fpl.input.id);
        if let Some(stat) = input_stats {
            stat.processed_stats.group_count = new_fpl.playlist.len();
            stat.processed_stats.channel_count = new_fpl.playlist.iter()
                .map(|group| group.channels.len())
                .sum();
        }

        new_fetched_playlists.push(new_fpl);
    });

    apply_affixes(&mut new_fetched_playlists);
    let mut new_playlist = vec![];
    new_fetched_playlists.iter_mut().for_each(|fp| {
        fp.playlist.drain(..).for_each(|group| new_playlist.push(group));
    });

    if !new_playlist.is_empty() {
        sort_playlist(target, &mut new_playlist);

        if target.watch.is_some() {
            if default_as_default().eq_ignore_ascii_case(&target.name) {
                error!("cant watch a target with no unique name");
            } else {
                let titles = target.watch.as_ref().unwrap();
                new_playlist.iter().for_each(|pl| {
                    if titles.contains(&pl.title) {
                        process_group_watch(cfg, &target.name, pl)
                    }
                });
            }
        }

        let publish = target.publish;
        if target.filename.is_some() {
            let result = write_playlist(target, cfg, &mut new_playlist);
            match &result {
                Ok(..) => {}
                Err(e) => {
                    error!("failed to write playlist {} {}", target.name, e);
                }
            }
            if !publish {
                return result;
            }
        }

        if target.publish {
            return xtream_save_playlist(target, cfg, &mut new_playlist);
        }
        create_m3u_filter_error_result!(M3uFilterErrorKind::Notify, "Persisting playlist failed: {}", &target.name)
    } else {
        info!("Playlist is empty: {}", &target.name);
        Ok(())
    }
}

pub(crate) fn start_processing(cfg: Arc<Config>, targets: Arc<ProcessTargets>) {
    let (stats, errors) = process_sources(cfg.clone(), targets.clone());
    let stats_msg = format!("Stats: {}", stats.iter().map(|stat| stat.to_string()).collect::<Vec<String>>().join("\n"));
    // print stats
    info!("{}", stats_msg);
    // send stats
    send_message(&MsgKind::Stats, &cfg.messaging, stats_msg.as_str());
    // log errors
    errors.iter().for_each(|err| error!("{}", err.message));
    // send errors
    if let Some(message) = get_errors_notify_message!(errors, 255) {
        send_message(&MsgKind::Error,&cfg.messaging, message.as_str());
    }
}