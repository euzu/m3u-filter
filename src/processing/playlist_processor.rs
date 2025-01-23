extern crate unidecode;

use crate::repository::storage::hash_string;
use async_std::sync::Mutex;
use core::cmp::Ordering;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::thread;

use actix_rt::System;
use log::{debug, error, info, log_enabled, trace, warn, Level};
use std::time::Instant;
use unidecode::unidecode;

use crate::filter::{get_field_value, set_field_value, MockValueProcessor, ValueProvider};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::messaging::{send_message, MsgKind};
use crate::model::config::{ConfigSortChannel, ConfigSortGroup, ConfigTarget, InputType,
                           ItemField, ProcessTargets, ProcessingOrder, SortOrder::{Asc, Desc}};
use crate::model::mapping::{CounterModifier, Mapping, MappingValueProcessor};
use crate::model::playlist::{FetchedPlaylist, FieldGetAccessor, FieldSetAccessor, PlaylistEntry, PlaylistGroup, PlaylistItem, UUIDType, XtreamCluster};
use crate::model::stats::{InputStats, PlaylistStats, SourceStats, TargetStats};
use crate::processing::affix_processor::apply_affixes;
use crate::processing::playlist_watch::process_group_watch;
use crate::processing::xmltv_parser::flatten_tvguide;
use crate::processing::xtream_processor_series::playlist_resolve_series;
use crate::processing::xtream_processor_vod::playlist_resolve_vod;
use crate::repository::playlist_repository::persist_playlist;
use crate::utils::default_utils::default_as_default;
use crate::utils::download;
use crate::utils::request_utils::sanitize_sensitive_info;
use crate::{debug_if_enabled, get_errors_notify_message, model::config, notify_err, Config};

fn is_valid(pli: &PlaylistItem, target: &ConfigTarget) -> bool {
    let provider = ValueProvider { pli: RefCell::new(pli) };
    target.filter(&provider)
}

#[allow(clippy::unnecessary_wraps)]
fn filter_playlist(playlist: &mut [PlaylistGroup], target: &ConfigTarget) -> Option<Vec<PlaylistGroup>> {
    debug!("Filtering {} groups", playlist.len());
    let mut new_playlist = Vec::with_capacity(128);
    for pg in playlist.iter_mut() {
        let channels = pg.channels.iter()
            .filter(|&pli| is_valid(pli, target)).cloned().collect::<Vec<PlaylistItem>>();
        trace!("Filtered group {} has now {}/{} items", pg.title, channels.len(), pg.channels.len());
        if !channels.is_empty() {
            new_playlist.push(PlaylistGroup {
                id: pg.id,
                title: pg.title.clone(),
                channels,
                xtream_cluster: pg.xtream_cluster,
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
    channel_sort.sequence.as_ref().map_or_else(|| {
        let ordering = value_a.partial_cmp(&value_b).unwrap();
        match channel_sort.order {
            Asc => ordering,
            Desc => ordering.reverse()
        }
    }, |custom_order| {
        // Check indices in the custom order vector
        let index_a = custom_order.iter().position(|s| s == value_a.as_ref());
        let index_b = custom_order.iter().position(|s| s == value_b.as_ref());

        match (index_a, index_b) {
            (Some(idx_a), Some(idx_b)) => {
                // Both items found in custom order, compare indices
                idx_a.cmp(&idx_b)
            }
            (Some(_), None) => {
                // Only 'a' found in custom order, it comes first
                Ordering::Less
            }
            (None, Some(_)) => {
                // Only 'b' found in custom order, it comes first
                Ordering::Greater
            }
            (None, None) => {
                // Neither found, fall back to default ordering
                let ordering = value_a.partial_cmp(&value_b).unwrap();
                match channel_sort.order {
                    Asc => ordering,
                    Desc => ordering.reverse(),
                }
            }
        }
    })
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

fn exec_rename(pli: &PlaylistItem, rename: Option<&Vec<config::ConfigRename>>) {
    if let Some(renames) = rename {
        if !renames.is_empty() {
            let result = pli;
            for r in renames {
                let value = get_field_value(result, &r.field);
                let cap = r.re.as_ref().unwrap().replace_all(value.as_str(), &r.new_name);
                if log::log_enabled!(log::Level::Debug) && *value != cap {
                    debug_if_enabled!("Renamed {}={} to {}", &r.field, value, cap);
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
                let mut new_playlist: Vec<PlaylistGroup> = Vec::with_capacity(playlist.len());
                for g in playlist {
                    let mut grp = g.clone();
                    for r in renames {
                        if matches!(r.field, ItemField::Group) {
                            let cap = r.re.as_ref().unwrap().replace_all(&grp.title, &r.new_name);
                            debug_if_enabled!("Renamed group {} to {} for {}", &grp.title, cap, target.name);
                            grp.title = Rc::new(cap.into_owned());
                        }
                    }

                    grp.channels.iter_mut().for_each(|pli| exec_rename(pli, target.rename.as_ref()));
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
        if mapping.match_as_ascii && log_enabled!(Level::Trace) { trace!("Decoded {} for matching to {}", &header.name, &channel_name); };
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
        let mut new_groups: Vec<PlaylistGroup> = Vec::with_capacity(128);
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
                        xtream_cluster: *cluster,
                    });
                }
            }
        }
        Some(new_groups)
    } else {
        None
    }
}

fn map_playlist_counter(target: &ConfigTarget, playlist: &[PlaylistGroup]) {
    if target.t_mapping.is_some() {
        let mut mock_processor = MockValueProcessor {};
        let mappings = target.t_mapping.as_ref().unwrap();
        for mapping in mappings {
            if let Some(counter_list) = &mapping.t_counter {
                for counter in counter_list {
                    let cntval = counter.value.load(core::sync::atomic::Ordering::Relaxed);
                    for plg in playlist {
                        for channel in &plg.channels {
                            let provider = ValueProvider { pli: RefCell::new(channel) };
                            if counter.filter.filter(&provider, &mut mock_processor) {
                                let new_value = if counter.modifier == CounterModifier::Assign {
                                    cntval.to_string()
                                } else {
                                    let value = channel.header.borrow_mut().get_field(&counter.field).map_or_else(String::new, |field_value| field_value.to_string());
                                    if counter.modifier == CounterModifier::Suffix {
                                        format!("{value}{}{cntval}", counter.concat)
                                    } else {
                                        format!("{cntval}{}{value}", counter.concat)
                                    }
                                };
                                channel.header.borrow_mut().set_field(&counter.field, new_value.as_str());
                                counter.value.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                            }
                        }
                    }
                }
            }
        }
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

async fn process_source(client: Arc<reqwest::Client>, cfg: Arc<Config>, source_idx: usize, user_targets: Arc<ProcessTargets>) -> (Vec<InputStats>, Vec<TargetStats>, Vec<M3uFilterError>) {
    let source = cfg.sources.get(source_idx).unwrap();
    let mut errors = vec![];
    let mut input_stats = HashMap::<u16, InputStats>::new();
    let mut target_stats = Vec::<TargetStats>::new();
    let mut source_playlists = Vec::with_capacity(128);
    let enabled_inputs = source.inputs.iter().filter(|item| item.enabled).count();
    // Downlod the sources
    for input in &source.inputs {
        let input_id = input.id;
        if is_input_enabled(enabled_inputs, input.enabled, input_id, &user_targets) {
            let start_time = Instant::now();
            let (mut playlistgroups, mut error_list) = match input.input_type {
                InputType::M3u => download::get_m3u_playlist(Arc::clone(&client), &cfg, input, &cfg.working_dir).await,
                InputType::Xtream => download::get_xtream_playlist(Arc::clone(&client), input, &cfg.working_dir).await,
            };
            let (tvguide, mut tvguide_errors) = if error_list.is_empty() {
                download::get_xmltv(Arc::clone(&client), &cfg, input, &cfg.working_dir).await
            } else {
                (None, vec![])
            };
            errors.append(&mut error_list);
            errors.append(&mut tvguide_errors);
            let group_count = playlistgroups.len();
            let channel_count = playlistgroups.iter()
                .map(|group| group.channels.len())
                .sum();
            let input_name = input.name.as_ref().map_or_else(|| sanitize_sensitive_info(input.url.as_str()), std::string::ToString::to_string);
            if playlistgroups.is_empty() {
                info!("Source is empty {input_name}");
                errors.push(notify_err!(format!("Source is empty {input_name}")));
            } else {
                playlistgroups.iter_mut().for_each(PlaylistGroup::on_load);
                source_playlists.push(
                    FetchedPlaylist {
                        input,
                        playlistgroups,
                        epg: tvguide,
                    }
                );
            }
            let elapsed = start_time.elapsed().as_secs();
            input_stats.insert(input_id, create_input_stat(group_count, channel_count, error_list.len(),
                                                           input.input_type.clone(), &input_name, elapsed));
        }
    }
    if source_playlists.is_empty() {
        debug!("Source at index {source_idx} is empty");
        errors.push(notify_err!(format!("Source at {source_idx} is empty")));
    } else {
        debug_if_enabled!("Source has {} groups", source_playlists.iter().map(|fpl| fpl.playlistgroups.len()).sum::<usize>());
        for target in &source.targets {
            if is_target_enabled(target, &user_targets) {
                match process_playlist_for_target(Arc::clone(&client), &mut source_playlists, target, &cfg, &mut input_stats, &mut errors).await {
                    Ok(()) => {
                        target_stats.push(TargetStats::success(&target.name));
                    }
                    Err(mut err) => {
                        target_stats.push(TargetStats::failure(&target.name));
                        errors.append(&mut err);
                    }
                }
            }
        }
    }
    (input_stats.into_values().collect(), target_stats, errors)
}

fn create_input_stat(group_count: usize, channel_count: usize, error_count: usize, input_type: InputType, input_name: &str, secs_took: u64) -> InputStats {
    InputStats {
        name: input_name.to_string(),
        input_type,
        error_count,
        raw_stats: PlaylistStats {
            group_count,
            channel_count,
        },
        processed_stats: PlaylistStats {
            group_count: 0,
            channel_count: 0,
        },
        secs_took,
    }
}

async fn process_sources(client: Arc<reqwest::Client>, config: Arc<Config>, user_targets: Arc<ProcessTargets>) -> (Vec<SourceStats>, Vec<M3uFilterError>) {
    let mut handle_list = vec![];
    let thread_num = config.threads;
    let process_parallel = thread_num > 1 && config.sources.len() > 1;
    if process_parallel && log_enabled!(Level::Debug) {
        debug!("Using {} threads", thread_num);
    }
    let errors = Arc::new(Mutex::<Vec<M3uFilterError>>::new(vec![]));
    let stats = Arc::new(Mutex::<Vec<SourceStats>>::new(vec![]));
    for (index, _) in config.sources.iter().enumerate() {
        // We're using the file lock this way on purpose
        let source_lock_path = PathBuf::from(format!("source_{index}"));
        let Ok(update_lock) = config.file_locks.try_write_lock(&source_lock_path).await else {
            warn!("The update operation for the source at index {index} was skipped because an update is already in progress.");
            continue;
        };

        let shared_errors = errors.clone();
        let shared_stats = stats.clone();
        let cfg = config.clone();
        let usr_trgts = user_targets.clone();
        if process_parallel {
            let http_client = Arc::clone(&client);
            let handles = &mut handle_list;
            let process = move || {
                System::new().block_on(async {
                    let (input_stats, target_stats, mut res_errors) = process_source(Arc::clone(&http_client), cfg, index, usr_trgts).await;
                    shared_errors.lock().await.append(&mut res_errors);
                    let process_stats = SourceStats::new(input_stats, target_stats);
                    shared_stats.lock().await.push(process_stats);
                });
            };
            handles.push(thread::spawn(process));
            if handles.len() >= thread_num as usize {
                handles.drain(..).for_each(|handle| { let _ = handle.join(); });
            }
        } else {
            let (input_stats, target_stats, mut res_errors) = process_source(Arc::clone(&client), cfg, index, usr_trgts).await;
            shared_errors.lock().await.append(&mut res_errors);
            let process_stats = SourceStats::new(input_stats, target_stats);
            shared_stats.lock().await.push(process_stats);
        }
        drop(update_lock);
    }
    for handle in handle_list {
        let _ = handle.join();
    }
    (Arc::try_unwrap(stats).unwrap().into_inner(), Arc::try_unwrap(errors).unwrap().into_inner())
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

fn duplicate_hash(item: &PlaylistItem) -> UUIDType {
    hash_string(&item.get_provider_url())
}

fn execute_pipe<'a>(target: &ConfigTarget, pipe: &ProcessingPipe, fpl: &FetchedPlaylist<'a>, duplicates: &mut HashSet<UUIDType>) -> FetchedPlaylist<'a> {
    let mut new_fpl = FetchedPlaylist {
        input: fpl.input,
        playlistgroups: fpl.playlistgroups.clone(), // we need to clone, because of multiple target definitions, we cant change the initial playlist.
        epg: fpl.epg.clone(),
    };
    if target.options.as_ref().is_some_and(|opt| opt.remove_duplicates) {
        for group in &mut new_fpl.playlistgroups {
            // `HashSet::insert`  returns true for first insert, otherweise false
            group.channels.retain(|item| duplicates.insert(duplicate_hash(item)));
        }
    }

    for f in pipe {
        if let Some(groups) = f(&mut new_fpl.playlistgroups, target) {
            new_fpl.playlistgroups = groups;
        }
    }
    new_fpl
}

// This method is needed, because of duplicate group names in different inputs.
// We merge the same group names considering cluster together.
fn flatten_groups(playlistgroups: Vec<PlaylistGroup>) -> Vec<PlaylistGroup> {
    let mut sort_order: Vec<PlaylistGroup> = vec![];
    let mut idx: usize = 0;
    let mut group_map: HashMap<(Rc<String>, XtreamCluster), usize> = HashMap::new();
    for group in playlistgroups {
        let key = (Rc::clone(&group.title), group.xtream_cluster);
        match group_map.entry(key) {
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(idx);
                idx += 1;
                sort_order.push(group);
            }
            std::collections::hash_map::Entry::Occupied(o) => {
                sort_order.get_mut(*o.get()).unwrap().channels.extend(group.channels);
            }
        };
    }
    sort_order
}

async fn process_playlist_for_target(client: Arc<reqwest::Client>,
                                     playlists: &mut [FetchedPlaylist<'_>],
                                     target: &ConfigTarget,
                                     cfg: &Config,
                                     stats: &mut HashMap<u16, InputStats>,
                                     errors: &mut Vec<M3uFilterError>) -> Result<(), Vec<M3uFilterError>> {
    let pipe = get_processing_pipe(target);
    debug_if_enabled!("Processing order is {}", &target.processing_order);

    let mut duplicates: HashSet<UUIDType> = HashSet::new();
    let mut processed_fetched_playlists: Vec<FetchedPlaylist> = vec![];
    for provider_fpl in playlists.iter_mut() {
        let mut processed_fpl = execute_pipe(target, &pipe, provider_fpl, &mut duplicates);
        playlist_resolve_series(Arc::clone(&client), cfg, target, errors, &pipe, provider_fpl, &mut processed_fpl).await;
        playlist_resolve_vod(Arc::clone(&client), cfg, target, errors, &processed_fpl).await;
        // stats
        let input_stats = stats.get_mut(&processed_fpl.input.id);
        if let Some(stat) = input_stats {
            stat.processed_stats.group_count = processed_fpl.playlistgroups.len();
            stat.processed_stats.channel_count = processed_fpl.playlistgroups.iter()
                .map(|group| group.channels.len())
                .sum();
        }
        processed_fetched_playlists.push(processed_fpl);
    }

    apply_affixes(&mut processed_fetched_playlists);

    let mut new_playlist = vec![];
    let mut new_epg = vec![];

    // each fetched playlist can have its own epgl url.
    // we need to process each input epg.
    for mut fp in processed_fetched_playlists {
        // collect all epg_channel ids
        let epg_channel_ids: HashSet<_> = fp.playlistgroups.iter().flat_map(|g| &g.channels)
            .filter_map(|c| c.header.borrow().epg_channel_id.clone()).collect();

        new_playlist.append(&mut fp.playlistgroups);
        if epg_channel_ids.is_empty() {
            debug_if_enabled!("channel ids are empty");
        } else if let Some(tv_guide) = fp.epg {
            debug!("found epg information for {}", &target.name);
            if let Some(epg) = tv_guide.filter(&epg_channel_ids) {
                new_epg.push(epg);
            }
        }
    }

    if new_playlist.is_empty() {
        info!("Playlist is empty: {}", &target.name);
        Ok(())
    } else {
        let mut flat_new_playlist = flatten_groups(new_playlist);
        sort_playlist(target, &mut flat_new_playlist);
        map_playlist_counter(target, &flat_new_playlist);
        process_watch(target, cfg, &flat_new_playlist);
        persist_playlist(&mut flat_new_playlist, flatten_tvguide(&new_epg).as_ref(), target, cfg).await
    }
}

fn process_watch(target: &ConfigTarget, cfg: &Config, new_playlist: &Vec<PlaylistGroup>) {
    if target.t_watch_re.is_some() {
        if default_as_default().eq_ignore_ascii_case(&target.name) {
            error!("cant watch a target with no unique name");
        } else {
            let watch_re = target.t_watch_re.as_ref().unwrap();
            for pl in new_playlist {
                if watch_re.iter().any(|r| r.is_match(&pl.title)) {
                    process_group_watch(cfg, &target.name, pl);
                }
            }
        }
    }
}

pub async fn exec_processing(client: Arc<reqwest::Client>, cfg: Arc<Config>, targets: Arc<ProcessTargets>) {
    let start_time = Instant::now();
    let (stats, errors) = process_sources(client, cfg.clone(), targets.clone()).await;
    // log errors
    for err in &errors {
        error!("{}", err.message);
    }
    if let Ok(stats_msg) = serde_json::to_string(&serde_json::Value::Object(serde_json::map::Map::from_iter([("stats".to_string(), serde_json::to_value(stats).unwrap())]))) {
        // print stats
        info!("{}", stats_msg);
        // send stats
        send_message(&MsgKind::Stats, cfg.messaging.as_ref(), stats_msg.as_str());
    }
    // send errors
    if let Some(message) = get_errors_notify_message!(errors, 255) {
        if let Ok(error_msg) = serde_json::to_string(&serde_json::Value::Object(serde_json::map::Map::from_iter([("errors".to_string(), serde_json::Value::String(message))]))) {
            send_message(&MsgKind::Error, cfg.messaging.as_ref(), error_msg.as_str());
        }
    }
    let elapsed = start_time.elapsed().as_secs();
    info!("Update process finished! Took {elapsed} secs.");
}