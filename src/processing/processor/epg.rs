use crate::model::config::{EpgConfig, EpgSmartMatchConfig};
use crate::model::playlist::{FetchedPlaylist, PlaylistItem, XtreamCluster};
use crate::model::xmltv::{Epg, XmlTag, EPG_ATTRIB_ID};
use crate::processing::parser::xmltv::normalize_channel_name;
use log::debug;
use rphonetic::{DoubleMetaphone, Encoder};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
pub struct EpgIdCache<'a> {
    pub channel_epg_id: HashSet<Cow<'a, str>>,
    pub normalized: HashMap<String, Option<String>>,
    pub phonetics: HashMap<String, HashSet<String>>,
    pub processed: HashSet<String>,
    pub smart_match_config: EpgSmartMatchConfig,
    pub metaphone: DoubleMetaphone,
    pub smart_match_enabled: bool, // smart match is enabled, normalizing names
    pub fuzzy_match_enabled: bool, // fuzzy matching enabled
}

impl EpgIdCache<'_> {
    pub fn new(epg_config: Option<&EpgConfig>) -> Self {
        let normalize_config = epg_config.map_or_else(EpgSmartMatchConfig::default, |epg_config| epg_config.t_smart_match.clone());
        EpgIdCache {
            channel_epg_id: HashSet::new(), // contains the epg_ids collected from playlist channels
            normalized: HashMap::new(),
            phonetics: HashMap::new(),
            processed: HashSet::new(),
            metaphone: DoubleMetaphone::default(),
            smart_match_enabled: normalize_config.enabled,
            fuzzy_match_enabled: normalize_config.enabled && normalize_config.fuzzy_matching,
            smart_match_config: normalize_config,

        }
    }

    fn is_empty(&self) -> bool {
        self.channel_epg_id.is_empty() && self.normalized.is_empty()
    }

    fn normalize_and_store(&mut self, name: &str, epg_id: Option<&String>) {
        let normalized_name = self.normalize(name);
        let phonetic = self.phonetic(&normalized_name);
        self.normalized.insert(normalized_name.to_string(), epg_id.map(std::string::ToString::to_string));
        self.phonetics.entry(phonetic.to_string()).or_default().insert(normalized_name);
    }

    fn normalize(&self, name: &str) -> String {
        normalize_channel_name(name, &self.smart_match_config)
    }

    pub(crate) fn phonetic(&self, name: &str) -> String {
        self.metaphone.encode(name)
    }

    pub fn collect_epg_id(&mut self, fp: &mut FetchedPlaylist) {
        let smart_match_enabled = self.smart_match_enabled;
        let fuzzy_matching = self.fuzzy_match_enabled;

        for channel in fp.playlistgroups.iter().flat_map(|g| &g.channels) {
            let mut missing_epg_id = true;
            // insert epg_id to known channel epg_ids
            if let Some(id) = channel.header.epg_channel_id.as_deref() {
                if !id.is_empty() {
                    missing_epg_id = false;
                    self.channel_epg_id.insert(Cow::Owned(id.to_string()));
                }
            }

            // for fuzzy_matching we need to put the normalized name even if there is an epg_id, because the epg_id
            // could not match to the epg file. And then we try to guess it based on normalized name
            let needs_normalization = smart_match_enabled && (fuzzy_matching || missing_epg_id);

            if needs_normalization {
                let name = &channel.header.name;
                self.normalize_and_store(name, channel.header.epg_channel_id.as_ref());
            }
        }
    }

    pub fn match_with_normalized(&mut self, epg_id: &str, normalized_epg_ids: &[String]) -> bool {
        for key in normalized_epg_ids {
            if let Some(entry) = self.normalized.get_mut(key) {
                entry.replace(epg_id.to_string());
                self.channel_epg_id.insert(epg_id.to_string().into());
                return true;
            }
        }
        false
    }
}

fn assign_channel_epg(new_epg: &mut Vec<Epg>, fp: &mut FetchedPlaylist, id_cache: &mut EpgIdCache) {
    if let Some(tv_guide) = &fp.epg {
        if let Some(epg) = tv_guide.filter(id_cache) {
            // // icon tags
            let icon_tags: HashMap<&String, &XmlTag> = epg.children.iter()
                .filter(|tag| tag.icon.is_some() && tag.get_attribute_value(EPG_ATTRIB_ID).is_some())
                .map(|t| (t.get_attribute_value(EPG_ATTRIB_ID).unwrap(), t)).collect();

            let filter_live = |c: &&mut PlaylistItem| c.header.xtream_cluster == XtreamCluster::Live;
            // let filter_missing_epg_id = |chan: &mut PlaylistItem| chan.header.epg_channel_id.is_none() || chan.header.logo.is_empty() || chan.header.logo_small.is_empty();
            let filter_missing_epg_id = |chan: &&mut PlaylistItem| chan.header.epg_channel_id.is_none();

            let assign_values = |chan: &mut PlaylistItem| {
                if id_cache.smart_match_enabled {
                    // if the channel has no epg_id  or the epg_id is not present in xmltv/tvguide then we need to match one from existing tvguide
                    let not_processed = match &chan.header.epg_channel_id {
                        None => true,
                        Some(epg_id) => !id_cache.processed.contains(epg_id),
                    };
                    if not_processed {
                        let normalized = id_cache.normalize(&chan.header.name);
                        if let Some(epg_id) = id_cache.normalized.get(&normalized) {
                            chan.header.epg_channel_id.clone_from(epg_id);
                        }
                    }
                }
                if chan.header.epg_channel_id.is_some() && (chan.header.logo.is_empty() || chan.header.logo_small.is_empty()) {
                    if let Some(icon_tag) = icon_tags.get(chan.header.epg_channel_id.as_ref().unwrap()) {
                        if let Some(icon) = icon_tag.icon.as_ref() {
                            if chan.header.logo.is_empty() {
                                chan.header.logo = (*icon).to_string();
                            }
                            if chan.header.logo_small.is_empty() {
                                chan.header.logo_small = (*icon).to_string();
                            }
                        }
                    }
                }
            };

            fp.playlistgroups.iter_mut()
                .flat_map(|g| &mut g.channels)
                .filter(filter_live)
                .filter(filter_missing_epg_id)
                .for_each(assign_values);
            new_epg.push(epg);
        }
    }
}

pub fn process_playlist_epg(fp: &mut FetchedPlaylist, epg: &mut Vec<Epg>) {
    // collect all epg_channel ids
    let mut id_cache = EpgIdCache::new(fp.input.epg.as_ref());
    id_cache.collect_epg_id(fp);

    if id_cache.is_empty() && !id_cache.smart_match_enabled {
        debug!("No epg ids found");
    } else {
        assign_channel_epg(epg, fp, &mut id_cache);
    }
}


#[cfg(test)]
mod tests {
    use rand::distr::Alphanumeric;
    use rand::Rng;
    use rphonetic::{DoubleMetaphone, Encoder};
    use tokio::time::Instant;

    fn random_string() -> String {
        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect()
    }

    #[test]
    fn test_phonetic() {
        let strings: Vec<String> = (0..5_000)
            .map(|_| random_string())
            .collect();

        let phonetic = DoubleMetaphone::new(Some(6));

        let now = Instant::now();
        for value in &strings {
            let _ = phonetic.encode(value);
        }

        let elapsed = now.elapsed();
        println!("Elapsed time: {}.{:03} secs", elapsed.as_secs(), elapsed.subsec_millis());
    }
}