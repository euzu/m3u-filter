use crate::model::config::{EpgNamePrefix, EpgSmartMatchConfig};
use crate::model::xmltv::{Epg, TVGuide, XmlTag, EPG_ATTRIB_CHANNEL, EPG_ATTRIB_ID, EPG_TAG_CHANNEL, EPG_TAG_DISPLAY_NAME, EPG_TAG_ICON, EPG_TAG_PROGRAMME, EPG_TAG_TV};
use crate::utils::compression::compressed_file_reader::CompressedFileReader;
use deunicode::deunicode;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::Reader;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::borrow::Cow;
use std::cmp::min;
use std::collections::{HashMap, HashSet};
use std::mem;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use crate::processing::processor::epg::EpgIdCache;
use crate::utils::constants::CONSTANTS;

fn split_by_first_match<'a>(input: &'a str, delimiters: &[char]) -> (Option<&'a str>, &'a str) {
    for delim in delimiters {
        if let Some(index) = input.find(*delim) {
            let (left, right) = input.split_at(index);
            let right = &right[delim.len_utf8()..].trim();
            if !right.is_empty() {
                let prefix = left.trim();
                // when we used anything as prefix the result was bad
                if CONSTANTS.country_codes.contains(&prefix) {
                    return (Some(prefix), right.trim());
                }
            }
        }
    }
    (None, input)
}

fn name_prefix<'a>(name: &'a str, smart_config: &EpgSmartMatchConfig) -> (&'a str, Option<&'a str>) {
    if smart_config.name_prefix != EpgNamePrefix::Ignore {
        let (prefix, suffix) = split_by_first_match(name, &smart_config.t_name_prefix_separator.clone());
        if prefix.is_some() {
            return (suffix, prefix);
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
pub fn normalize_channel_name(name: &str, normalize_config: &EpgSmartMatchConfig) -> String {
    let normalized = deunicode(name.trim()).to_lowercase();
    let (channel_name, suffix) = name_prefix(&normalized, normalize_config);
    // Remove all non-alphanumeric characters (except dashes and underscores).
    let cleaned_name = normalize_config.t_normalize_regex.as_ref().unwrap().replace_all(channel_name, "");
    // Remove terms like resolution
    let cleaned_name = normalize_config.t_strip.iter().fold(cleaned_name.to_string(), |acc, term| {
        acc.replace(term, "")
    });
    match suffix {
        None => cleaned_name,
        Some(sfx) => {
            match &normalize_config.name_prefix {
                EpgNamePrefix::Ignore => cleaned_name,
                EpgNamePrefix::Suffix(sep) => combine(sep, &cleaned_name, sfx),
                EpgNamePrefix::Prefix(sep) => combine(sep, sfx, &cleaned_name),
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

    fn prepare_tag(id_cache: &mut EpgIdCache, tag: &mut XmlTag, smart_match: bool) {
        if let Some(children) = &mut tag.children {
            for child in children {
                match child.name.as_str() {
                    EPG_TAG_DISPLAY_NAME => {
                        if smart_match {
                            if let Some(name) = &child.value {
                                tag.normalized_epg_ids.push(normalize_channel_name(name, &id_cache.smart_match_config));
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
    }

    fn try_fuzzy_matching(id_cache: &mut EpgIdCache, epg_id: &str, tag: &XmlTag, fuzzy_matching: bool) -> bool {
        let mut matched = id_cache.match_with_normalized(epg_id, &tag.normalized_epg_ids);
        if !matched && fuzzy_matching {
            let (fuzzy_matched, matched_normalized_name) = Self::find_best_fuzzy_match(id_cache, tag);
            if fuzzy_matched {
                let key = matched_normalized_name.unwrap();
                let id = epg_id.to_string();
                id_cache.normalized.entry(key).and_modify(|entry| {
                    entry.replace(id.clone());
                    id_cache.channel_epg_id.insert(Cow::Owned(id));
                    matched = true;
                });
            }
        }
        matched
    }

    fn find_best_fuzzy_match(id_cache: &mut EpgIdCache, tag: &XmlTag) -> (bool, Option<String>) {
        let early_exit_flag = Arc::new(AtomicBool::new(false)); // Flag für den frühen Abbruch
        let data: Mutex<(u16, Option<Cow<str>>)> = Mutex::new((0, None));

        let match_threshold = id_cache.smart_match_config.match_threshold;
        let best_match_threshold = id_cache.smart_match_config.best_match_threshold;

        id_cache.normalized.par_iter().for_each(|(norm_key, _epg_id_opt)| {
            if early_exit_flag.load(Ordering::SeqCst) {
                return;
            }
            let phonetic_code = id_cache.phonetic(norm_key);
            for normalized_epg_id in &tag.normalized_epg_ids {
                let code = id_cache.phonetic(normalized_epg_id);
                if code == phonetic_code {
                    let match_jw = strsim::jaro_winkler(norm_key, normalized_epg_id);
                    #[allow(clippy::cast_possible_truncation)]
                    #[allow(clippy::cast_sign_loss)]
                    let mjw = min(100, match_jw as u16 * 100);
                    if mjw >= match_threshold {
                        let mut lock = data.lock().unwrap();
                        if lock.0 < mjw {
                            *lock = (mjw, Some(Cow::Borrowed(norm_key)));
                        }
                        if mjw > best_match_threshold {
                            early_exit_flag.store(true, Ordering::SeqCst);
                            return; // (true, matched_normalized_epg_id.map(|s| s.to_string()));
                        }
                    }
                }
            }
            // is there an early exit strategy ???
        });
        if early_exit_flag.load(Ordering::SeqCst) {
            let result = data.lock().unwrap().1.take();
            return (true, result.as_ref().map(std::string::ToString::to_string));
        }
        (false, None)
    }

    fn process_epg_file(id_cache: &mut EpgIdCache, epg_file: &Path) -> Option<Epg> {
        match CompressedFileReader::new(epg_file) {
            Ok(mut reader) => {
                let mut children: Vec<XmlTag> = vec![];
                let mut tv_attributes: Option<HashMap<String, String>> = None;
                let smart_match = id_cache.smart_match_config.enabled;
                let fuzzy_matching = smart_match && id_cache.smart_match_config.fuzzy_matching;
                let mut filter_tags = |mut tag: XmlTag| {
                    match tag.name.as_str() {
                        EPG_TAG_CHANNEL => {
                            Self::prepare_tag(id_cache, &mut tag, smart_match);
                            if let Some(epg_id) = tag.get_attribute_value(EPG_ATTRIB_ID) {
                                if !id_cache.processed.contains(epg_id) {
                                    if smart_match {
                                        if Self::try_fuzzy_matching(id_cache, epg_id, &tag, fuzzy_matching) {
                                            children.push(tag);
                                        }
                                    } else {
                                        let borrowed_epg_id = Cow::Borrowed(epg_id.as_str());
                                        if id_cache.channel_epg_id.contains(&borrowed_epg_id) {
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
                                    if id_cache.channel_epg_id.contains(&borrowed_epg_id) {
                                        children.push(tag);
                                    }
                                }
                            }
                        }
                        EPG_TAG_TV => {
                            tv_attributes = tag.attributes.take();
                        }
                        _ => {}
                    }
                };

                parse_tvguide(&mut reader, &mut filter_tags);

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
        if id_cache.channel_epg_id.is_empty() && id_cache.normalized.is_empty() {
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


fn handle_tag_start<F>(callback: &mut F, stack: &mut Vec<XmlTag>, e: &BytesStart)
where
    F: FnMut(XmlTag),
{
    let name = String::from_utf8_lossy(e.name().as_ref()).as_ref().to_owned();
    let (is_tv_tag, is_channel, is_program) = get_tag_types(&name);
    let attributes = collect_tag_attributes(e, is_channel, is_program);
    let attribs = if attributes.is_empty() { None } else { Some(attributes) };
    let tag = XmlTag::new(name, attribs);

    if is_tv_tag {
        callback(tag);
    } else {
        stack.push(tag);
    }
}


fn handle_tag_end<F>(callback: &mut F, stack: &mut Vec<XmlTag>)
where
    F: FnMut(XmlTag),
{
    if !stack.is_empty() {
        if let Some(tag) = stack.pop() {
            if tag.name == EPG_TAG_CHANNEL {
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
                    r.children = Some(match r.children.take() {
                        None => vec![tag],
                        Some(mut tags) => {
                            tags.push(tag);
                            tags
                        }
                    });
                    r
                }) {
                    stack.push(old_tag);
                }
            }
        }
    }
}


fn handle_text_tag(stack: &mut [XmlTag], e: &BytesText) {
    if !stack.is_empty() {
        if let Ok(text) = e.unescape() {
            let t = text.trim();
            if !t.is_empty() {
                stack.last_mut().unwrap().value = Some(t.to_string());
            }
        }
    }
}

pub fn parse_tvguide<R, F>(content: R, callback: &mut F)
where
    R: std::io::BufRead,
    F: FnMut(XmlTag),
{
    let mut stack: Vec<XmlTag> = vec![];
    let mut reader = Reader::from_reader(content);
    let mut buf = Vec::<u8>::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => handle_tag_start(callback, &mut stack, &e),
            Ok(Event::End(_e)) => handle_tag_end(callback, &mut stack),
            Ok(Event::Text(e)) => handle_text_tag(&mut stack, &e),
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
        let epg_children = Mutex::new(Vec::new());
        let epg_attributes = tv_guides.first().and_then(|t| t.attributes.clone());
        let count = tv_guides.iter().map(|tvg| tvg.children.len()).sum();
        let channel_ids: RwLock<HashSet<&String>> = RwLock::new(HashSet::with_capacity(count));
        tv_guides.par_iter().for_each(|guide| {
            let mut children = vec![];
            guide.children.iter().for_each(|c| {
                if c.name.as_str() == EPG_TAG_CHANNEL {
                    if let Some(chan_id) = c.get_attribute_value(EPG_ATTRIB_ID) {
                        let channel_id_exists = channel_ids.read().unwrap().contains(&chan_id);
                        if !channel_id_exists {
                            channel_ids.write().unwrap().insert(chan_id);
                            children.push(c.clone());
                        }
                    }
                }
            });
            guide.children.iter().for_each(|c| {
                if c.name.as_str() == EPG_TAG_PROGRAMME {
                    if let Some(chan_id) = c.get_attribute_value(EPG_TAG_CHANNEL) {
                        if channel_ids.read().unwrap().contains(&chan_id) {
                            children.push(c.clone());
                        }
                    }
                }
            });

            epg_children.lock().unwrap().extend(children);
        });
        let epg = Epg {
            attributes: epg_attributes,
            children: mem::take(&mut *epg_children.lock().unwrap()),
        };
        Some(epg)
    }
}


#[cfg(test)]
mod tests {
    use crate::model::config::{EpgNamePrefix, EpgSmartMatchConfig};
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
        let mut epg_smart_cfg = EpgSmartMatchConfig::default();
        epg_smart_cfg.enabled = true;
        epg_smart_cfg.name_prefix = EpgNamePrefix::Suffix(".".to_string());
        let _ = epg_smart_cfg.prepare();
        println!("{:?}", epg_smart_cfg);
        assert_eq!("supersport6.ru", normalize_channel_name("RU: SUPERSPORT 6 ᴿᴬᵂ", &epg_smart_cfg));
        assert_eq!("odisea.sat", normalize_channel_name("SAT: ODISEA ᴿᴬᵂ", &epg_smart_cfg));
        assert_eq!("odisea.4k", normalize_channel_name("4K: ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_smart_cfg));
        assert_eq!("odisea", normalize_channel_name("ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_smart_cfg));
        assert_eq!("odisea.bu", normalize_channel_name("BU | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_smart_cfg));
        assert_eq!("odisea.bg", normalize_channel_name("BG | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_smart_cfg));
    }

    use rphonetic::{Encoder, Metaphone};
    #[test]
    fn test_metaphone() {
        let metaphone = Metaphone::default();
        let mut epg_smart_cfg = EpgSmartMatchConfig::default();
        epg_smart_cfg.enabled = true;
        epg_smart_cfg.name_prefix = EpgNamePrefix::Suffix(".".to_string());
        let _ = epg_smart_cfg.prepare();
        println!("{:?}", epg_smart_cfg);
        // assert_eq!("supersport6.ru", metaphone.encode(&normalize_channel_name("RU: SUPERSPORT 6 ᴿᴬᵂ", &epg_normalize_cfg)));
        // assert_eq!("odisea.sat", metaphone.encode(&normalize_channel_name("SAT: ODISEA ᴿᴬᵂ", &epg_normalize_cfg)));
        // assert_eq!("odisea", metaphone.encode(&normalize_channel_name("4K: ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
        // assert_eq!("odisea", metaphone.encode(&normalize_channel_name("ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
        // assert_eq!("odisea.bu", metaphone.encode(&normalize_channel_name("BU | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));
        // assert_eq!("odisea.bg", metaphone.encode(&normalize_channel_name("BG | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_normalize_cfg)));

        println!("{}", metaphone.encode(&normalize_channel_name("RU: SUPERSPORT 6 ᴿᴬᵂ", &epg_smart_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("SAT: ODISEA ᴿᴬᵂ", &epg_smart_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("4K: ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_smart_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_smart_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("BU | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_smart_cfg)));
        println!("{}", metaphone.encode(&normalize_channel_name("BG | ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ", &epg_smart_cfg)));
    }
}