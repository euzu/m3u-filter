use crate::model::config::{EpgCountryPrefix, EpgNormalizeConfig};
use crate::model::xmltv::{Epg, TVGuide, XmlTag, EPG_ATTRIB_CHANNEL, EPG_ATTRIB_ID, EPG_TAG_CHANNEL, EPG_TAG_DISPLAY_NAME, EPG_TAG_ICON, EPG_TAG_PROGRAMME, EPG_TAG_TV};
use crate::processing::processor::playlist::EpgIdCache;
use crate::utils::compression::compressed_file_reader::CompressedFileReader;
use deunicode::deunicode;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, LazyLock};
use rphonetic::{Encoder, Metaphone};

static COUNTRY_CODE: LazyLock<HashSet<&'static str>> = LazyLock::new(|| vec![
    "af", "al", "dz", "ad", "ao", "ag", "ar", "am", "au", "at", "az", "bs", "bh", "bd", "bb", "by",
    "be", "bz", "bj", "bt", "bo", "ba", "bw", "br", "bn", "bg", "bf", "bi", "cv", "kh", "cm", "ca",
    "cf", "td", "cl", "cn", "co", "km", "cg", "cr", "hr", "cu", "cy", "cz", "cd", "dk", "dj", "dm",
    "do", "tl", "ec", "eg", "sv", "gq", "er", "ee", "sz", "et", "fj", "fi", "fr", "ga", "gm", "ge",
    "de", "gh", "gr", "gd", "gt", "gn", "gw", "gy", "ht", "hn", "hu", "is", "in", "id", "ir", "iq",
    "ie", "il", "it", "ci", "jm", "jp", "jo", "kz", "ke", "ki", "kp", "kr", "kw", "kg", "la", "lv",
    "lb", "ls", "lr", "ly", "li", "lt", "lu", "mg", "mw", "my", "mv", "ml", "mt", "mh", "mr", "mu",
    "mx", "fm", "md", "mc", "mn", "me", "ma", "mz", "mm", "na", "nr", "np", "nl", "nz", "ni", "ne",
    "ng", "mk", "no", "om", "pk", "pw", "pa", "pg", "py", "pe", "ph", "pl", "pt", "qa", "ro", "ru",
    "rw", "kn", "lc", "vc", "ws", "sm", "st", "sa", "sn", "rs", "sc", "sl", "sg", "sk", "si", "sb",
    "so", "za", "ss", "es", "lk", "sd", "sr", "se", "ch", "sy", "tw", "tj", "tz", "th", "tg", "to",
    "tt", "tn", "tr", "tm", "tv", "ug", "ua", "ae", "gb", "us", "uy", "uz", "vu", "va", "ve", "vn",
    "ye", "zm", "zw",
].into_iter().collect::<HashSet<&str>>());

fn split_country_prefix(s: &str) -> (Option<String>, &str) {
    if s.len() < 5 ||  !s[2..3].chars().any(|c| !c.is_alphabetic()) {
        return (None, s);
    }
    let first_code = &s[0..2];
    if !COUNTRY_CODE.contains(first_code) {
        return (None, s);
    }
    (Some(first_code.to_string()), &s[3..])
}

fn country_prefix(name: String, normalize_config: &EpgNormalizeConfig) -> (String, Option<String>) {
    if normalize_config.country_prefix != EpgCountryPrefix::Ignore {
        let (prefix, suffix) = split_country_prefix(&name);
        if prefix.is_some() {
            return (suffix.to_string(), prefix);
        }
    }
    (name, None)
}

fn combine(join: &str, left: &str, right: &str) -> String {
    let mut combined = String::with_capacity(left.len() + join.len() + right.len());
    combined.push_str(left);
    combined.push('.');
    combined.push_str(right);
    combined
}

/// # Panics
pub fn normalize_channel_name(name: &str, normalize_config: &EpgNormalizeConfig) -> String {
    let normalized = deunicode(name).to_lowercase();
    let (channel_name, suffix) = country_prefix(normalized, normalize_config);
    // Remove all non-alphanumeric characters (except dashes and underscores).
    let cleaned_name = normalize_config.t_normalize_regex.as_ref().unwrap().replace_all(&channel_name, "");
    // Remove terms like resolution
    let cleaned_name = normalize_config.t_strip.iter().fold(cleaned_name.to_string(), |acc, term| {
        acc.replace(term, "")
    });
    match suffix {
        None => cleaned_name,
        Some(sfx) => {
            match &normalize_config.country_prefix {
                EpgCountryPrefix::Ignore => cleaned_name,
                EpgCountryPrefix::Suffix(sep) => combine(sep, &cleaned_name, &sfx),
                EpgCountryPrefix::Prefix(sep) => combine(sep, &sfx, &cleaned_name),
            }
        }
    }
}

impl TVGuide {
    fn merge(mut epgs: Vec<Epg>) -> Option<Epg> {
        if epgs.is_empty() {
            return None;
        }
        let first_epg_attributes = epgs.get_mut(0).unwrap().attributes.take();
        let merged_children: Vec<XmlTag> = epgs.into_iter().flat_map(|epg| epg.children).collect();
        Some(Epg {
            attributes: first_epg_attributes,
            children: merged_children,
        })
    }

    fn process_epg_file(id_cache: &mut EpgIdCache, epg_file: &Path) -> Option<Epg> {
        match CompressedFileReader::new(epg_file) {
            Ok(mut reader) => {
                let mut children: Vec<XmlTag> = vec![];
                let mut tv_attributes: Option<Arc<HashMap<String, String>>> = None;
                let metaphone = Metaphone::default();
                let normalization = id_cache.normalize_config.enabled;
                let fuzzy_matching = id_cache.normalize_config.fuzzy_matching;
                let mut filter_tags = |tag: XmlTag| {
                    match tag.name.as_str() {
                        EPG_TAG_CHANNEL => {
                            if let Some(epg_id) = tag.get_attribute_value(EPG_ATTRIB_ID) {
                                if !id_cache.processed.contains(epg_id) {
                                    if normalization {
                                        let mut matched = false;
                                        let id: Cow<str> = Cow::Owned(epg_id.to_string());
                                        for normalized_epg_id in &tag.normalized_epg_ids {
                                            let key = Cow::Owned(normalized_epg_id.to_string());
                                            match id_cache.normalized.entry(key) {
                                                std::collections::hash_map::Entry::Occupied(mut entry) => {
                                                    entry.get_mut().1 = Some(id.clone());
                                                    id_cache.channel.insert(id.clone());
                                                    matched = true;
                                                    break;
                                                }
                                                std::collections::hash_map::Entry::Vacant(_entry) => {}
                                            }
                                        }

                                        if !matched && fuzzy_matching {
                                            let mut matched_normalized_epg_id: Option<Cow<str>> = None;
                                            let mut threshold = 0.0;

                                            'outer: for (norm_key, (phonetic_code, _)) in &id_cache.normalized {
                                                for normalized_epg_id in &tag.normalized_epg_ids {
                                                    let code = metaphone.encode(normalized_epg_id);
                                                    if &code == phonetic_code {
                                                        let match_jw = strsim::jaro_winkler(norm_key, normalized_epg_id);
                                                        if match_jw >= id_cache.normalize_config.t_match_threshold {
                                                            threshold = if threshold > match_jw { threshold } else {
                                                                matched_normalized_epg_id = Some(Cow::Borrowed(norm_key));
                                                                match_jw
                                                            };
                                                            if threshold > 99.9 {
                                                                break 'outer;
                                                            }
                                                        }
                                                    }
                                                }
                                                // is there an early exit strategy ???
                                            }
                                            if matched {
                                                match id_cache.normalized.entry(Cow::Owned(matched_normalized_epg_id.unwrap().to_string())) {
                                                    std::collections::hash_map::Entry::Occupied(mut entry) => {
                                                        entry.get_mut().1 = Some(id.clone());
                                                        id_cache.channel.insert(id.clone());
                                                        matched = true;
                                                    }
                                                    std::collections::hash_map::Entry::Vacant(_entry) => {}
                                                }
                                            }
                                        }

                                        if matched {
                                            children.push(tag);
                                        }
                                    } else {
                                        let borrowed_epg_id = Cow::Borrowed(epg_id.as_str());
                                        if id_cache.channel.contains(&borrowed_epg_id) {
                                            children.push(tag);
                                        }
                                    }
                                }
                            }
                        }
                        EPG_TAG_PROGRAMME => {
                            if let Some(epg_id) = tag.get_attribute_value(EPG_ATTRIB_CHANNEL) {
                                if !id_cache.processed.contains(epg_id) {
                                    let borrowed_epg_id = Cow::Borrowed(epg_id.as_str());
                                    if id_cache.channel.contains(&borrowed_epg_id) {
                                        children.push(tag);
                                    }
                                }
                            }
                        }
                        EPG_TAG_TV => {
                            tv_attributes.clone_from(&tag.attributes);
                        }
                        _ => {}
                    }
                };

                parse_tvguide(&mut reader, &mut filter_tags, &id_cache.normalize_config);

                if children.is_empty() {
                    return None;
                }

                children.iter().filter(|tag| tag.name == EPG_TAG_CHANNEL).for_each(|tag| {
                    if let Some(epg_id) = tag.get_attribute_value(EPG_ATTRIB_ID) {
                        id_cache.processed.insert(epg_id.to_string());
                    }
                });

                Some(Epg {
                    attributes: tv_attributes,
                    children,
                })
            }
            Err(_) => None
        }
    }

    pub fn filter(&self, id_cache: &mut EpgIdCache) -> Option<Epg> {
        if id_cache.channel.is_empty() && id_cache.normalized.is_empty() {
            return None;
        }
        let epgs: Vec<Epg> = self.file_paths.iter()
            .filter_map(|path| Self::process_epg_file(id_cache, path))
            .collect();
        if epgs.len() == 1 {
            epgs.into_iter().next()
        } else {
            Self::merge(epgs)
        }
    }
}

pub fn parse_tvguide<R, F>(content: R, callback: &mut F, epg_normalize_config: &EpgNormalizeConfig)
where
    R: std::io::BufRead,
    F: FnMut(XmlTag),
{
    let mut stack: Vec<XmlTag> = vec![];
    let mut reader = Reader::from_reader(content);
    let mut buf = Vec::<u8>::new();
    let normalize_enabled = epg_normalize_config.enabled;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).as_ref().to_owned();
                let (is_tv_tag, is_channel, is_program) = get_tag_types(&name);
                let attributes = collect_tag_attributes(&e, is_channel, is_program);
                let attribs = if attributes.is_empty() { None } else { Some(Arc::new(attributes)) };
                let tag = XmlTag {
                    name,
                    value: None,
                    attributes: attribs,
                    children: None,
                    icon: None,
                    normalized_epg_ids: HashSet::new(),
                };

                if is_tv_tag {
                    callback(tag);
                } else {
                    stack.push(tag);
                }
            }
            Ok(Event::End(_e)) => {
                if !stack.is_empty() {
                    if let Some(mut tag) = stack.pop() {
                        if tag.name == EPG_TAG_CHANNEL {
                            if let Some(children) = &mut tag.children {
                                for child in children {
                                    match child.name.as_str() {
                                        EPG_TAG_DISPLAY_NAME => {
                                            if normalize_enabled {
                                                if  let Some(name) = &child.value {
                                                    tag.normalized_epg_ids.insert(normalize_channel_name(name, epg_normalize_config));
                                                }
                                            }
                                        }
                                        EPG_TAG_ICON => {
                                            if let Some(src) = child.get_attribute_value("src") {
                                                tag.icon = Some(src.to_string());
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            if let Some(chan_id) = tag.get_attribute_value(EPG_ATTRIB_ID) {
                                if !chan_id.is_empty() {
                                    callback(tag);
                                }
                            }
                        } else if tag.name == EPG_TAG_PROGRAMME {
                            if let Some(chan_id) = tag.get_attribute_value(EPG_ATTRIB_CHANNEL) {
                                if !chan_id.is_empty() {
                                    callback(tag);
                                }
                            }
                        } else if !stack.is_empty() {
                            if let Some(old_tag) = stack.pop().map(|mut r| {
                                let rc_tag = Arc::new(tag);
                                r.children = Some(
                                    r.children.map_or_else(|| vec![Arc::clone(&rc_tag)], |mut c| {
                                        c.push(Arc::clone(&rc_tag));
                                        c
                                    }));
                                r
                            }) {
                                stack.push(old_tag);
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(e)) => {
                if !stack.is_empty() {
                    if let Ok(text) = e.unescape() {
                        let t = text.trim();
                        if !t.is_empty() {
                            stack.last_mut().unwrap().value = Some(t.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn get_tag_types(name: &str) -> (bool, bool, bool) {
    let (is_tv_tag, is_channel, is_program) = match name {
        EPG_TAG_TV => (true, false, false),
        EPG_TAG_CHANNEL => (false, true, false),
        EPG_TAG_PROGRAMME => (false, false, true),
        _ => (false, false, false)
    };
    (is_tv_tag, is_channel, is_program)
}

fn collect_tag_attributes(e: &BytesStart, is_channel: bool, is_program: bool) -> HashMap<String, String> {
    let attributes = e.attributes().filter_map(Result::ok)
        .filter_map(|a| {
            let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
            let mut value = String::from(a.unescape_value().unwrap().as_ref());
            if (is_channel && key == EPG_ATTRIB_ID) || (is_program && key == EPG_ATTRIB_CHANNEL) {
                value = value.to_lowercase().to_string();
            }
            if value.is_empty() {
                None
            } else {
                Some((key, value))
            }
        }).collect::<HashMap<String, String>>();
    attributes
}

pub fn flatten_tvguide(tv_guides: &[Epg]) -> Option<Epg> {
    if tv_guides.is_empty() {
        None
    } else {
        let mut epg = Epg {
            attributes: None,
            children: vec![],
        };
        let count = tv_guides.iter().map(|tvg| tvg.children.len()).sum();
        let mut channel_ids: HashSet<&String> = HashSet::with_capacity(count);
        for guide in tv_guides {
            if epg.attributes.is_none() {
                epg.attributes.clone_from(&guide.attributes);
            }
            guide.children.iter().for_each(|c| {
                if c.name.as_str() == EPG_TAG_CHANNEL {
                    if let Some(chan_id) = c.get_attribute_value(EPG_ATTRIB_ID) {
                        if !channel_ids.contains(&chan_id) {
                            channel_ids.insert(chan_id);
                            epg.children.push(c.clone());
                        }
                    }
                }
            });
            guide.children.iter().for_each(|c| {
                if c.name.as_str() == EPG_TAG_PROGRAMME {
                    if let Some(chan_id) = c.get_attribute_value(EPG_TAG_CHANNEL) {
                        if channel_ids.contains(&chan_id) {
                            epg.children.push(c.clone());
                        }
                    }
                }
            });
        }
        Some(epg)
    }
}


#[cfg(test)]
mod tests {
    use crate::model::config::{EpgCountryPrefix, EpgNormalizeConfig};
    use crate::processing::parser::xmltv::normalize_channel_name;

    // #[test]
    // fn parse_test() -> io::Result<()> {
    //     let file_path = PathBuf::from("/tmp/epg.xml.gz");
    //
    //     if file_path.exists() {
    //         let tv_guide = TVGuide { file: file_path };
    //
    //         let mut channel_ids = HashSet::from(["channel.1".to_string(), "channel.2".to_string(), "channel.3".to_string()]);
    //         let mut nomalized = HashMap::new();
    //         match tv_guide.filter(&mut channel_ids, &mut nomalized) {
    //             None => assert!(false, "No epg filtered"),
    //             Some(epg) => {
    //                 assert_eq!(epg.children.len(), channel_ids.len() * 2, "Epg size does not match")
    //             }
    //         }
    //     }
    //     Ok(())
    // }

    #[test]
    fn normalize() {
        let mut epg_normalize_cfg = EpgNormalizeConfig::default();
        epg_normalize_cfg.country_prefix = EpgCountryPrefix::Suffix(".".to_string());
        println!("{:?}", epg_normalize_cfg);
        assert_eq!("supersport6.ru", normalize_channel_name("RU: SUPERSPORT 6 ᴿᴬᵂ", &epg_normalize_cfg));
        assert_eq!("satodisea", normalize_channel_name("SAT: ODISEA ᴿᴬᵂ", &epg_normalize_cfg));
        assert_eq!("odisea", normalize_channel_name("4K: ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg));
        assert_eq!("odisea", normalize_channel_name("ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg));
        assert_eq!("buodisea", normalize_channel_name("BU | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg));
        assert_eq!("odisea.bg", normalize_channel_name("BG | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg));
    }

    use rphonetic::{Encoder, Metaphone};
    #[test]
    fn test_metaphone() {
        let metaphone = Metaphone::default();
        let mut epg_normalize_cfg = EpgNormalizeConfig::default();
        epg_normalize_cfg.country_prefix = EpgCountryPrefix::Suffix(".".to_string());
        println!("{:?}", epg_normalize_cfg);
        // assert_eq!("supersport6.ru", metaphone.encode(&normalize_channel_name("RU: SUPERSPORT 6 ᴿᴬᵂ", &epg_normalize_cfg)));
        // assert_eq!("satodisea", metaphone.encode(&normalize_channel_name("SAT: ODISEA ᴿᴬᵂ", &epg_normalize_cfg)));
        // assert_eq!("odisea", metaphone.encode(&normalize_channel_name("4K: ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
        // assert_eq!("odisea", metaphone.encode(&normalize_channel_name("ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
        // assert_eq!("buodisea", metaphone.encode(&normalize_channel_name("BU | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
        // assert_eq!("odisea.bg", metaphone.encode(&normalize_channel_name("BG | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));

        println!("{}", metaphone.encode(&normalize_channel_name("RU: SUPERSPORT 6 ᴿᴬᵂ", &epg_normalize_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("SAT: ODISEA ᴿᴬᵂ", &epg_normalize_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("4K: ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("BU | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("BG | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
    }
}