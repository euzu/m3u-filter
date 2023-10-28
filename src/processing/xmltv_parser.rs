use std::collections::HashMap;
use std::rc::Rc;
use quick_xml::events::Event;
use quick_xml::Reader;
use crate::model::xmltv::{Epg, TVGuide, XmlTag};

pub(crate) fn parse_tvguide(content: &str) -> Option<TVGuide> {
    let mut stack: Vec<XmlTag> = vec![];
    let mut reader = Reader::from_str(content);
    reader.trim_text(true);
    let mut buf = Vec::<u8>::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let attributes = e.attributes().filter_map(|a| a.ok())
                    .filter_map(|a| {
                        let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                        let value = String::from(a.unescape_value().unwrap().as_ref()).to_string();
                        if !value.is_empty() {
                            Some((key, value))
                        } else {
                            None
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
        let mut program_ids: Vec<&String> = vec![];

            tv_guides.iter().for_each(|guide| {
           if epg.attributes.is_none() {
               epg.attributes = guide.attributes.clone();
           }
           guide.children.iter().for_each(|c| {
               let add_child = match c.name.as_str() {
                   "channel" => {
                       let chan_id = c.get_attribute_value("id");
                       match chan_id {
                           Some(id) => {
                               if channel_ids.contains(&id) {
                                   false
                               } else {
                                   channel_ids.push(id);
                                   true
                               }
                           },
                           None => false,
                       }
                   },
                   "programme" => {
                       let chan_id = c.get_attribute_value("channel");
                       match chan_id {
                           Some(id) => {
                               if program_ids.contains(&id) {
                                   false
                               } else {
                                   program_ids.push(id);
                                   true
                               }
                           },
                           None => false,
                       }
                   }
                   _ => false,
               };
               if add_child {
                   epg.children.push(c.clone())
               }
           });
        });
        Some(epg)
    }
}