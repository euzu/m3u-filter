use std::collections::HashMap;
use std::rc::Rc;
use quick_xml::events::Event;
use quick_xml::Reader;
use crate::model::xmltv::{Epg, TVGuide, XmlTag};

static EPG_PROGRAMME: &str = "programme";
static EPG_CHANNEL: &str = "channel";
static EPG_ID: &str = "id";

pub(crate) fn parse_tvguide(content: &str) -> Option<TVGuide> {
    let mut stack: Vec<XmlTag> = vec![];
    let mut reader = Reader::from_str(content);
    let mut buf = Vec::<u8>::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let attributes = e.attributes().filter_map(std::result::Result::ok)
                    .filter_map(|a| {
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let value = String::from(a.unescape_value().unwrap().as_ref()).to_string();
                        if value.is_empty() {
                            None
                        } else {
                            Some((key, value))
                        }
                    }).collect::<HashMap<String, String>>();
                let tag = XmlTag {
                    name,
                    value: None,
                    attributes: if attributes.is_empty() { None } else { Some(Rc::new(attributes)) },
                    children: None,
                };

                stack.push(tag);
            }
            Ok(Event::End(_e)) => {
                if stack.len() > 1 {
                    if let Some(tag) = stack.pop() {
                        if let Some(old_tag) = stack.pop().map(|mut r| {
                            let rc_tag = Rc::new(tag);
                            r.children = Some(
                                r.children.map_or(vec![rc_tag.clone()],
                                                  |mut c| {
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
            Ok(Event::Text(e)) => {
                let text = e.unescape().unwrap().into_owned();
                if !text.is_empty() {
                    stack.last_mut().unwrap().value = Some(text);
                }
            }
            _ => {}
        }
    }
    stack.pop().map(|epg| TVGuide {
        epg,
    })
}

pub(crate) fn flatten_tvguide(tv_guides: &[Epg]) -> Option<Epg> {
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
                if c.name.as_str() == EPG_CHANNEL {
                    if let Some(chan_id) = c.get_attribute_value(EPG_ID) {
                        if !channel_ids.contains(&chan_id) {
                            channel_ids.push(chan_id);
                            epg.children.push(c.clone());
                        }
                    }
                }
            });
            guide.children.iter().for_each(|c| {
                if c.name.as_str() == EPG_PROGRAMME {
                    if let Some(chan_id) = c.get_attribute_value(EPG_CHANNEL) {
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