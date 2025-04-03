use std::collections::{HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use quick_xml::{Writer};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Error;
use quick_xml::Reader;

pub const EPG_TAG_TV: &str = "tv";
pub const EPG_TAG_PROGRAMME: &str = "programme";
pub const EPG_TAG_CHANNEL: &str = "channel";
pub const EPG_ATTRIB_ID: &str = "id";
pub const EPG_ATTRIB_CHANNEL: &str = "channel";

// https://github.com/XMLTV/xmltv/blob/master/xmltv.dtd

#[derive(Debug, Clone)]
pub struct XmlTag {
    pub name: String,
    pub value: Option<String>,
    pub attributes: Option<Arc<HashMap<String, String>>>,
    pub children: Option<Vec<Arc<XmlTag>>>,
}

impl XmlTag {
    pub fn get_attribute_value(&self, attr_name: &str) -> Option<&String> {
        self.attributes.as_ref().and_then(|attr| attr.get(attr_name))
    }

    fn write_to<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<(), Error> {
        let mut elem = BytesStart::new(self.name.as_str());
        if let Some(attribs) = self.attributes.as_ref() {
            attribs.iter().for_each(|(k, v)| elem.push_attribute((k.as_str(), v.as_str())));
        }
        writer.write_event(Event::Start(elem))?;
        self.value.as_ref().map(|text| writer.write_event(Event::Text(BytesText::new(text.as_str()))));
        if let Some(children) = &self.children {
            for child in children {
                child.write_to(writer)?;
            }
        }
        Ok(writer.write_event(Event::End(BytesEnd::new(self.name.as_str())))?)
    }

    pub fn parse_root<R: std::io::BufRead>(mut reader: R) -> Result<Epg, quick_xml::Error> {
        let mut xml = Reader::from_reader(&mut reader);
        //xml.trim_text(true);
        let mut buf = Vec::new();

        let _children: Vec<XmlTag> = Vec::new();
        while let Ok(event) = xml.read_event_into(&mut buf) {
            match event {
                Event::Start(start) if start.name().as_ref() == b"tv" => {
                    // Parse <tv> tag
                    let epg_children = Self::parse_children(&mut xml)?;
                    return Ok(Epg {
                        attributes: None,
                        children: epg_children,
                    });
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }


        Err(quick_xml::Error::Io(std::sync::Arc::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Expected  <tv>  root",
        ))))
    }

    fn parse_children<R: std::io::BufRead>(xml: &mut Reader<R>) -> Result<Vec<XmlTag>, quick_xml::Error> {
        let mut children = Vec::new();
        let mut buf = Vec::new();
    
        loop {
            match xml.read_event_into(&mut buf)? {
                Event::Start(e) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let mut text = None;
                    let mut tag_buf = Vec::new();
                    let mut sub_children = vec![];
    
                    loop {
                        match xml.read_event_into(&mut tag_buf)? {
                            Event::Text(t) => {
                                text = Some(t.unescape()?.to_string());
                            }
                            Event::Start(sub_start) => {
                                let sub_name = String::from_utf8_lossy(sub_start.name().as_ref()).to_string();
                                let mut sub_text = None;
                                let mut inner_buf = Vec::new();
                                let mut inner_children = vec![];
    
                                loop {
                                    match xml.read_event_into(&mut inner_buf)? {
                                        Event::Text(t) => {
                                            sub_text = Some(t.unescape()?.to_string());
                                        }
                                        Event::Start(_) => {
                                            let inner = Self::parse_children(xml)?;
                                            inner_children.extend(inner);
                                        }
                                        Event::End(end) if end.name().as_ref() == sub_start.name().as_ref() => break,
                                        Event::Eof => break,
                                        _ => {}
                                    }
                                    inner_buf.clear();
                                }
    
                                sub_children.push(XmlTag {
                                    name: sub_name,
                                    value: sub_text,
                                    attributes: Some(Arc::new({
                                        let mut map = HashMap::new();
                                        for attr in sub_start.attributes().with_checks(false) {
                                            if let Ok(attr) = attr {
                                                let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                                let val = attr.unescape_value()?.to_string();
                                                map.insert(key, val);
                                            }
                                        }
                                        map
                                    })),
                                    children: if inner_children.is_empty() {
                                        None
                                    } else {
                                        Some(inner_children.into_iter().map(Arc::new).collect())
                                    },
                                });
                            }
                            Event::End(end) if end.name().as_ref() == e.name().as_ref() => break,
                            Event::Eof => break,
                            _ => {}
                        }
                        tag_buf.clear();
                    }
    
                    children.push(XmlTag {
                        name,
                        value: text,
                        attributes: Some(Arc::new({
                            let mut map = HashMap::new();
                            for attr in e.attributes().with_checks(false) {
                                if let Ok(attr) = attr {
                                    let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                                    let val = attr.unescape_value()?.to_string();
                                    map.insert(key, val);
                                }
                            }
                            map
                        })),
                        children: if sub_children.is_empty() {
                            None
                        } else {
                            Some(sub_children.into_iter().map(Arc::new).collect())
                        },
                    });
                }
                Event::End(_) | Event::Eof => break,
                _ => {}
            }
    
            buf.clear();
        }
    
        Ok(children)
    }
}


#[derive(Debug, Clone)]
pub struct Epg {
    pub attributes: Option<Arc<HashMap<String, String>>>,
    pub children: Vec<XmlTag>,
}

impl Epg {
    pub fn write_to<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<(), quick_xml::Error> {
        let mut elem = BytesStart::new("tv");
        if let Some(attribs) = self.attributes.as_ref() {
            attribs.iter().for_each(|(k, v)| elem.push_attribute((k.as_str(), v.as_str())));
        }
        writer.write_event(Event::Start(elem))?;
        for child in &self.children {
            child.write_to(writer)?;
        }
        Ok(writer.write_event(Event::End(BytesEnd::new("tv")))?)
    }
}

#[derive(Debug, Clone)]
pub struct TVGuide {
    pub file: PathBuf,
}
