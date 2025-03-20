use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::model::xmltv::{Epg, EPG_ATTRIB_CHANNEL, EPG_ATTRIB_ID, EPG_TAG_TV, EPG_TAG_CHANNEL, EPG_TAG_PROGRAMME, TVGuide, XmlTag};
use crate::utils::compression::compressed_file_reader::CompressedFileReader;

impl TVGuide {
    pub fn filter(&self, channel_ids: &HashSet<String>) -> Option<Epg> {
        if channel_ids.is_empty() {
            return None;
        }
        match CompressedFileReader::new(&self.file) {
            Ok(mut reader) => {
                let mut children: Vec<XmlTag> = vec![];
                let mut tv_attributes: Option<Arc<HashMap<String, String>>> = None;
                let mut filter_tags = |tag: XmlTag| {
                    if match tag.name.as_str() {
                        EPG_TAG_CHANNEL => {
                            tag.get_attribute_value(EPG_ATTRIB_ID).is_some_and(|val| channel_ids.contains(val))
                        }
                        EPG_TAG_PROGRAMME => {
                            tag.get_attribute_value(EPG_ATTRIB_CHANNEL).is_some_and(|val| channel_ids.contains(val))
                        },
                        EPG_TAG_TV => {
                            tv_attributes.clone_from(&tag.attributes);
                            false
                        },
                        _ => false,
                    } {
                        children.push(tag);
                    }
                };

                parse_tvguide(&mut reader, &mut filter_tags);

                if children.is_empty() {
                    return None;
                }
                Some(Epg {
                    attributes: tv_attributes,
                    children,
                })
            }
            Err(_) => None
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
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let is_tv_tag = name == EPG_TAG_TV;
                let attributes = e.attributes().filter_map(Result::ok)
                    .filter_map(|a| {
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let value = String::from(a.unescape_value().unwrap().as_ref());
                        if value.is_empty() {
                            None
                        } else {
                            Some((key, value))
                        }
                    }).collect::<HashMap<String, String>>();
                let tag = XmlTag {
                    name,
                    value: None,
                    attributes: if attributes.is_empty() { None } else { Some(Arc::new(attributes)) },
                    children: None,
                };

                if is_tv_tag {
                    callback(tag);
                } else {
                    stack.push(tag);
                }
            }
            Ok(Event::End(_e)) => {
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
                                let rc_tag = Arc::new(tag);
                                r.children = Some(
                                    r.children.map_or_else(|| vec![rc_tag.clone()], |mut c| {
                                                          c.push(rc_tag.clone());
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
                let text = e.unescape().unwrap().trim().to_owned();
                if !text.is_empty() && !stack.is_empty() {
                    stack.last_mut().unwrap().value = Some(text);
                }
            }
            _ => {}
        }
    }
}

pub fn flatten_tvguide(tv_guides: &[Epg]) -> Option<Epg> {
    if tv_guides.is_empty() {
        None
    } else {
        let mut epg = Epg {
            attributes: None,
            children: vec![],
        };
        let mut channel_ids: Vec<&String> = vec![];
        for guide in tv_guides {
            if epg.attributes.is_none() {
                epg.attributes.clone_from(&guide.attributes);
            }
            guide.children.iter().for_each(|c| {
                if c.name.as_str() == EPG_TAG_CHANNEL {
                    if let Some(chan_id) = c.get_attribute_value(EPG_ATTRIB_ID) {
                        if !channel_ids.contains(&chan_id) {
                            channel_ids.push(chan_id);
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
    use std::collections::HashSet;
    use std::io;
    use std::path::PathBuf;
    use crate::model::xmltv::{TVGuide};

    #[test]
    fn parse_test() -> io::Result<()> {
        let file_path = PathBuf::from("/tmp/epg.xml.gz");

        if file_path.exists() {
            let tv_guide = TVGuide { file:  file_path};

            let channel_ids = vec!["channel.1", "channel.2", "channel.3"];
            let channel_ids : HashSet<String> =  channel_ids.into_iter().map(|s| s.to_string()).collect();

            match tv_guide.filter(&channel_ids) {
                None => assert!(false, "No epg filtered"),
                Some(epg) => {
                    assert_eq!(epg.children.len(), channel_ids.len() * 2, "Epg size does not match")
                }
            }
        }
        Ok(())
    }
}