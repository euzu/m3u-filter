use std::borrow::Cow;
use crate::model::xmltv::{Epg, TVGuide, XmlTag, EPG_ATTRIB_CHANNEL, EPG_ATTRIB_ID, EPG_TAG_CHANNEL, EPG_TAG_DISPLAY_NAME, EPG_TAG_ICON, EPG_TAG_PROGRAMME, EPG_TAG_TV};
use crate::utils::compression::compressed_file_reader::CompressedFileReader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, LazyLock};
use deunicode::deunicode;

static NORMALIZE_CHANNEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^a-zA-Z0-9\-]").unwrap());

// TODO into config
const TERMS_TO_REMOVE: &[&str] = &["3840p", "uhd", "fhd", "hd", "sd", "4k", "plus", "raw"];

pub fn normalize_channel_name(name: &str) -> String {
    let normalized = deunicode(name).to_lowercase();

    // Remove all non-alphanumeric characters (except dashes and underscores).
    let cleaned_name = NORMALIZE_CHANNEL.replace_all(&normalized, "");

    // Remove terms like resolution
    let result = TERMS_TO_REMOVE.iter().fold(cleaned_name.to_string(), |acc, term| {
        acc.replace(*term, "")
    });

    result
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

    fn process_epg_file(epg_channel_ids: &mut HashSet<Cow<str>>, normalized_epg_channel_ids: &mut HashMap<Cow<str>, Option<Cow<str>>>,
                        processed_epg_channel_ids: &mut HashSet<String>, epg_file: &Path) -> Option<Epg> {
        match CompressedFileReader::new(epg_file) {
            Ok(mut reader) => {
                let mut children: Vec<XmlTag> = vec![];
                let mut tv_attributes: Option<Arc<HashMap<String, String>>> = None;
                let mut filter_tags = |tag: XmlTag| {
                    match tag.name.as_str() {
                        EPG_TAG_CHANNEL => {
                            if let Some(epg_id) = tag.get_attribute_value(EPG_ATTRIB_ID) {
                                if !processed_epg_channel_ids.contains(epg_id) {
                                    let id: Cow<str> = Cow::Owned(epg_id.to_string());
                                    for normalized_epg_id in &tag.normalized_epg_ids {
                                        let key = Cow::Owned(normalized_epg_id.to_string());
                                        match normalized_epg_channel_ids.entry(key) {
                                            std::collections::hash_map::Entry::Occupied(mut entry) => {
                                                entry.insert(Some(id.clone()));
                                                epg_channel_ids.insert(id.clone());
                                            }
                                            std::collections::hash_map::Entry::Vacant(_entry) => {}
                                        }
                                    }
                                    if epg_channel_ids.contains(&id) {
                                        children.push(tag);
                                    }
                                }
                            }
                        }
                        EPG_TAG_PROGRAMME => {
                            if let Some(epg_id) = tag.get_attribute_value(EPG_ATTRIB_CHANNEL) {
                                if !processed_epg_channel_ids.contains(epg_id) {
                                    let borrowed_epg_id = Cow::Borrowed(epg_id.as_str());
                                    if epg_channel_ids.contains(&borrowed_epg_id) {
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

                parse_tvguide(&mut reader, &mut filter_tags);

                if children.is_empty() {
                    return None;
                }

                children.iter().filter(|tag| tag.name == EPG_TAG_CHANNEL).for_each(|tag| {
                    if let Some(epg_id) = tag.get_attribute_value(EPG_ATTRIB_ID) {
                        processed_epg_channel_ids.insert(epg_id.to_string());
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

    pub fn filter(&self, epg_channel_ids: &mut HashSet<Cow<str>>, normalized_epg_channel_ids: &mut HashMap<Cow<str>, Option<Cow<str>>>) -> Option<Epg> {
        if epg_channel_ids.is_empty() && normalized_epg_channel_ids.is_empty() {
            return None;
        }
        let mut processed_epg_ids: HashSet<String> = HashSet::new();
        let epgs: Vec<Epg> = self.file_paths.iter()
            .filter_map(|path| Self::process_epg_file(epg_channel_ids, normalized_epg_channel_ids, &mut processed_epg_ids, path))
            .collect();
        if epgs.len() == 1 {
            epgs.into_iter().next()
        } else {
            Self::merge(epgs)
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
                                            if let Some(name) = &child.value {
                                                tag.normalized_epg_ids.insert(normalize_channel_name(name));
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
        assert_eq!("satsupersport6", normalize_channel_name("SAT: SUPERSPORT 6 ᴿᴬᵂ"));
        assert_eq!("odisea", normalize_channel_name("4K: ODISEA ᴿᴬᵂ"));
        assert_eq!("odisea", normalize_channel_name("4K: ODISEA ᵁᴴᴰ ³⁸⁴⁰ᴾ"));

    }
}