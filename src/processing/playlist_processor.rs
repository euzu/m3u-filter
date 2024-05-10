extern crate unidecode;

use core::cmp::Ordering;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread;

use actix_rt::System;
use log::{debug, error, info, Level, log_enabled};
use unidecode::unidecode;

use crate::{Config, get_errors_notify_message, model::config};
use crate::filter::{get_field_value, MockValueProcessor, set_field_value, ValueProvider};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::messaging::{MsgKind, send_message};
use crate::model::config::{ConfigSortChannel, ConfigSortGroup, ConfigTarget, InputType,
                           ItemField, ProcessingOrder, ProcessTargets, SortOrder::{Asc, Desc}};
use crate::model::mapping::{Mapping, MappingValueProcessor};
use crate::model::playlist::{FetchedPlaylist, PlaylistGroup, PlaylistItem};
use crate::model::stats::{InputStats, PlaylistStats};
use crate::processing::affix_processor::apply_affixes;
use crate::processing::playlist_watch::process_group_watch;
use crate::processing::xmltv_parser::flatten_tvguide;
use crate::processing::xtream_processor::playlist_resolve_series;
use crate::repository::playlist_repository::persist_playlist;
use crate::utils::download;
use crate::utils::default_utils::default_as_default;

fn is_valid(pli: &PlaylistItem, target: &ConfigTarget) -> bool {
    let provider = ValueProvider { pli: RefCell::new(pli) };
    target.filter(&provider)
}

#[allow(clippy::unnecessary_wraps)]
fn filter_playlist(playlist: &mut [PlaylistGroup], target: &ConfigTarget) -> Option<Vec<PlaylistGroup>> {
    debug!("Filtering {} groups", playlist.len());
    let mut new_playlist = Vec::new();
    for pg in playlist.iter_mut() {
        let channels = pg.channels.iter()
            .filter(|&pli| is_valid(pli, target)).cloned().collect::<Vec<PlaylistItem>>();
        debug!("Filtered group {} has now {}/{} items", pg.title, channels.len(), pg.channels.len());
        if !channels.is_empty() {
            new_playlist.push(PlaylistGroup {
                id: pg.id,
                title: pg.title.clone(),
                channels,
                xtream_cluster: pg.xtream_cluster.clone(),
            });
        }
    }
    Some(new_playlist)
}

fn playlistgroup_comparator(a: &PlaylistGroup, b: &PlaylistGroup, group_sort: &ConfigSortGroup, match_as_ascii: bool) -> Ordering {
    let value_a = if match_as_ascii { Rc::new(unidecode(&a.title)) } else { Rc::clone(&a.title) };
    let value_b = if match_as_ascii { Rc::new(unidecode(&b.title)) } else { Rc::clone(&b.title) };
    let ordering = value_a.partial_cmp(&value_b).unwrap();
    match group_sort.order {
        Asc => ordering,
        Desc => ordering.reverse()
    }
}

fn playlistitem_comparator(a: &PlaylistItem, b: &PlaylistItem, channel_sort: &ConfigSortChannel, match_as_ascii: bool) -> Ordering {
    let raw_value_a = get_field_value(a, &channel_sort.field);
    let raw_value_b = get_field_value(b, &channel_sort.field);
    let value_a = if match_as_ascii { Rc::new(unidecode(&raw_value_a)) } else { raw_value_a };
    let value_b = if match_as_ascii { Rc::new(unidecode(&raw_value_b)) } else { raw_value_b };
    let ordering = value_a.partial_cmp(&value_b).unwrap();
    match channel_sort.order {
        Asc => ordering,
        Desc => ordering.reverse()
    }
}

fn sort_playlist(target: &ConfigTarget, new_playlist: &mut [PlaylistGroup]) {
    if let Some(sort) = &target.sort {
        let match_as_ascii = sort.match_as_ascii;
        if let Some(group_sort) = &sort.groups {
            new_playlist.sort_by(|a, b| playlistgroup_comparator(a, b, group_sort, match_as_ascii));
        }
        if let Some(channel_sorts) = &sort.channels {
            for channel_sort in channel_sorts {
                let regexp = channel_sort.re.as_ref().unwrap();
                for group in new_playlist.iter_mut() {
                    let group_title = if match_as_ascii { Rc::new(unidecode(&group.title)) } else { Rc::clone(&group.title) };
                    if regexp.is_match(group_title.as_str()) {
                        group.channels.sort_by(|chan1, chan2| playlistitem_comparator(chan1, chan2, channel_sort, match_as_ascii));
                    }
                }
            }
        }
    }
}

fn exec_rename(pli: &mut PlaylistItem, rename: &Option<Vec<config::ConfigRename>>) {
    if let Some(renames) = rename {
        if !renames.is_empty() {
            let result = pli;
            for r in renames {
                let value = get_field_value(result, &r.field);
                let cap = r.re.as_ref().unwrap().replace_all(value.as_str(), &r.new_name);
                if log_enabled!(Level::Debug) {
                    debug!("Renamed {}={} to {}", &r.field, value, cap);
                }
                let value = cap.into_owned();
                set_field_value(result, &r.field, Rc::new(value));
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
                            if log_enabled!(Level::Debug) {
                                debug!("Renamed group {} to {} for {}", &grp.title, cap, target.name);
                            }
                            grp.title = Rc::new(cap.into_owned());
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
            if let Some(ptrn) = $pattern {
               ptrn.filter($provider, $processor);
            };
    }};
}

fn map_channel(channel: PlaylistItem, mapping: &Mapping) -> PlaylistItem {
    if !mapping.mapper.is_empty() {
        let header = channel.header.borrow();
        let channel_name = if mapping.match_as_ascii { Rc::new(unidecode(&header.name)) } else { header.name.clone() };
        if mapping.match_as_ascii && log_enabled!(Level::Debug) { debug!("Decoded {} for matching to {}", &header.name, &channel_name); };
        drop(header);
        let ref_chan = RefCell::new(&channel);
        let provider = ValueProvider { pli: ref_chan.clone() };
        let mut mock_processor = MockValueProcessor {};
        for m in &mapping.mapper {
            let mut processor = MappingValueProcessor { pli: ref_chan.clone(), mapper: m };
            match &m.t_filter {
                Some(filter) => {
                    if filter.filter(&provider, &mut mock_processor) {
                        apply_pattern!(&m.t_pattern, &provider, &mut processor);
                    }
                }
                _ => {
                    apply_pattern!(&m.t_pattern, &provider, &mut processor);
                }
            };
        }
    }
    channel
}

fn map_playlist(playlist: &mut [PlaylistGroup], target: &ConfigTarget) -> Option<Vec<PlaylistGroup>> {
    if target.t_mapping.is_some() {
        let new_playlist: Vec<PlaylistGroup> = playlist.iter().map(|playlist_group| {
            let mut grp = playlist_group.clone();
            let mappings = target.t_mapping.as_ref().unwrap();
            mappings.iter().filter(|&mapping| !mapping.mapper.is_empty()).for_each(|mapping|
                grp.channels = grp.channels.drain(..).map(|chan| map_channel(chan, mapping)).collect());
            grp
        }).collect();

        // if the group names are changed, restructure channels to the right groups
        // we use
        let mut new_groups: Vec<PlaylistGroup> = Vec::new();
        let mut grp_id: u32 = 0;
        for playlist_group in new_playlist {
            for channel in &playlist_group.channels {
                let cluster = &channel.header.borrow().xtream_cluster;
                let title = &channel.header.borrow().group;
                if let Some(grp) = new_groups.iter_mut().find(|x| *x.title == **title) {
                    grp.channels.push(channel.clone());
                } else {
                    grp_id += 1;
                    new_groups.push(PlaylistGroup {
                        id: grp_id,
                        title: Rc::clone(title),
                        channels: vec![channel.clone()],
                        xtream_cluster: cluster.clone(),
                    });
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

fn is_target_enabled(target: &ConfigTarget, user_targets: &ProcessTargets) -> bool {
    (!user_targets.enabled && target.enabled) || (user_targets.enabled && user_targets.has_target(target.id))
}

async fn process_source(cfg: Arc<Config>, source_idx: usize, user_targets: Arc<ProcessTargets>) -> (Vec<InputStats>, Vec<M3uFilterError>) {
    let source = cfg.sources.get(source_idx).unwrap();
    let mut all_playlist = Vec::new();
    let enabled_inputs = source.inputs.iter().filter(|item| item.enabled).count();
    let mut errors = vec![];
    let mut stats = HashMap::<u16, InputStats>::new();
    for input in &source.inputs {
        let input_id = input.id;
        if is_input_enabled(enabled_inputs, input.enabled, input_id, &user_targets) {
            let (playlist, mut error_list) = match input.input_type {
                InputType::M3u => download::get_m3u_playlist(&cfg, input, &cfg.working_dir).await,
                InputType::Xtream => download::get_xtream_playlist(input, &cfg.working_dir).await,
            };
            let (tvguide, mut tvguide_errors) = if error_list.is_empty() {
                download::get_xmltv(&cfg, input, &cfg.working_dir).await
            } else {
                (None, vec![])
            };
            error_list.drain(..).for_each(|err| errors.push(err));
            tvguide_errors.drain(..).for_each(|err| errors.push(err));
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
                errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("source is empty {input_name}")));
            } else {
                all_playlist.push(
                    FetchedPlaylist {
                        input,
                        playlist,
                        epg: tvguide,
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
        if log_enabled!(Level::Debug) {
            debug!("Source at index {} input is empty", source_idx);
        }
        errors.push(M3uFilterError::new(M3uFilterErrorKind::Notify, format!("Source at {source_idx} input is empty")));
    } else {
        if log_enabled!(Level::Debug) {
            debug!("Input has {} groups", all_playlist.len());
        }
        for target in &source.targets {
            if is_target_enabled(target, &user_targets) {
                match process_playlist(&mut all_playlist, target, &cfg, &mut stats, &mut errors).await {
                    Ok(()) => {}
                    Err(mut err) => err.drain(..).for_each(|e| errors.push(e))
                }
            }
        }
    }
    (stats.drain().map(|(_, v)| v).collect(), errors)
}

async fn process_sources(config: Arc<Config>, user_targets: Arc<ProcessTargets>) -> (Vec<InputStats>, Vec<M3uFilterError>) {
    let mut handle_list = vec![];
    let thread_num = config.threads;
    let process_parallel = thread_num > 1 && config.sources.len() > 1;
    if process_parallel && log_enabled!(Level::Debug) {
        debug!("Using {} threads", thread_num);
    }
    let errors = Arc::new(Mutex::<Vec<M3uFilterError>>::new(vec![]));
    let stats = Arc::new(Mutex::<Vec<InputStats>>::new(vec![]));
    for (index, _) in config.sources.iter().enumerate() {
        let shared_errors = errors.clone();
        let shared_stats = stats.clone();
        let cfg = config.clone();
        let usr_trgts = user_targets.clone();
        if process_parallel {
            let handles = &mut handle_list;
            let process = move || {
                let (mut res_stats, mut res_errors) = System::new().block_on(async {
                    process_source(cfg, index, usr_trgts).await
                });
                res_errors.drain(..)
                    .for_each(|err| shared_errors.lock().unwrap().push(err));
                res_stats.drain(..)
                    .for_each(|stat| shared_stats.lock().unwrap().push(stat));
            };
            handles.push(thread::spawn(process));
            if handles.len() >= thread_num as usize {
                handles.drain(..).for_each(|handle| { let _ = handle.join(); });
            }
        } else {
            let (mut res_stats, mut res_errors) = process_source(cfg, index, usr_trgts).await;
            res_errors.drain(..)
                .for_each(|err| shared_errors.lock().unwrap().push(err));
            res_stats.drain(..)
                .for_each(|stat| shared_stats.lock().unwrap().push(stat));
        }
    }
    for handle in handle_list {
        let _ = handle.join();
    }
    (Arc::try_unwrap(stats).unwrap().into_inner().unwrap(), Arc::try_unwrap(errors).unwrap().into_inner().unwrap())
}

pub type ProcessingPipe = Vec<fn(playlist: &mut [PlaylistGroup], target: &ConfigTarget) -> Option<Vec<PlaylistGroup>>>;

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

async fn process_playlist<'a>(playlists: &mut [FetchedPlaylist<'a>],
                              target: &ConfigTarget, cfg: &Config,
                              stats: &mut HashMap<u16, InputStats>,
                              errors: &mut Vec<M3uFilterError>) -> Result<(), Vec<M3uFilterError>> {
    let pipe = get_processing_pipe(target);
    if log_enabled!(Level::Debug) {
        debug!("Processing order is {}", &target.processing_order);
    }

    let mut new_fetched_playlists: Vec<FetchedPlaylist> = vec![];
    for fpl in playlists.iter_mut() {
        let mut new_fpl = FetchedPlaylist {
            input: fpl.input,
            playlist: fpl.playlist.clone(), // we need to clone, because of multiple target definitions, we cant change the initial playlist.
            epg: fpl.epg.clone(),
        };
        for f in &pipe {
            let playlist = &mut new_fpl.playlist;
            let r = f(playlist, target);
            if let Some(v) = r {
                new_fpl.playlist = v;
            }
        }
        playlist_resolve_series(target, errors, &pipe, fpl, &mut new_fpl).await;
        // stats
        let input_stats = stats.get_mut(&new_fpl.input.id);
        if let Some(stat) = input_stats {
            stat.processed_stats.group_count = new_fpl.playlist.len();
            stat.processed_stats.channel_count = new_fpl.playlist.iter()
                .map(|group| group.channels.len())
                .sum();
        }

        new_fetched_playlists.push(new_fpl);
    }

    apply_affixes(&mut new_fetched_playlists);

    let mut new_playlist = vec![];
    let mut new_epg = vec![];

    new_fetched_playlists.drain(..).for_each(|mut fp| {
        let epg_channel_ids: HashSet<_> = fp.playlist.iter().flat_map(|g| &g.channels)
            .filter_map(|c| c.header.borrow().epg_channel_id.clone()).collect();
        fp.playlist.drain(..).for_each(|group| new_playlist.push(group));
        if !epg_channel_ids.is_empty() {
            if let Some(tv_guide) = fp.epg {
                debug!("found epg information for {}", &target.name);
                if let Some(epg) = tv_guide.filter(&epg_channel_ids) {
                    new_epg.push(epg);
                }
            }
        } else if log_enabled!(Level::Debug) {
            debug!("channel ids are empty");
        }
    });

    if new_playlist.is_empty() {
        info!("Playlist is empty: {}", &target.name);
        Ok(())
    } else {
        sort_playlist(target, &mut new_playlist);

        if target.t_watch_re.is_some() {
            if default_as_default().eq_ignore_ascii_case(&target.name) {
                error!("cant watch a target with no unique name");
            } else {
                let watch_re = target.t_watch_re.as_ref().unwrap();
                for pl in &new_playlist {
                    if watch_re.iter().any(|r| r.is_match(&pl.title)) {
                        process_group_watch(cfg, &target.name, pl);
                    }
                }
            }
        }

        persist_playlist(&mut new_playlist, flatten_tvguide(&new_epg).as_ref(), target, cfg)
    }
}

pub(crate) async fn exec_processing(cfg: Arc<Config>, targets: Arc<ProcessTargets>) {
    let (stats, errors) = process_sources(cfg.clone(), targets.clone()).await;
    let stats_msg = format!("{{\"stats\": {}}}", stats.iter().map(std::string::ToString::to_string).collect::<Vec<String>>().join("\n"));
    // print stats
    info!("{}", stats_msg);
    // send stats
    send_message(&MsgKind::Stats, &cfg.messaging, stats_msg.as_str());
    // log errors
    for err in &errors {
        error!("{}", err.message);
    }
    // send errors
    if let Some(message) = get_errors_notify_message!(errors, 255) {
        let error_msg = format!("{{\"errors\": \"{}\"}}", message.as_str());
        send_message(&MsgKind::Error, &cfg.messaging, error_msg.as_str());
    }
}