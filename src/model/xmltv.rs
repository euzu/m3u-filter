use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Error, Writer};

// https://github.com/XMLTV/xmltv/blob/master/xmltv.dtd


#[derive(Debug, Clone)]
pub(crate) struct XmlTag {
    pub name: String,
    pub value: Option<String>,
    pub attributes: Option<Rc<HashMap<String, String>>>,
    pub children: Option<Vec<Rc<XmlTag>>>,
}

impl XmlTag {
    pub(crate) fn get_attribute_value(&self, attr_name: &str) -> Option<&String> {
        match &self.attributes {
            None => None,
            Some(attr) => {
                attr.get(attr_name)
            }
        }
    }

    fn write_to<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<(), Error> {
        let mut elem = BytesStart::new(self.name.as_str());
        if let Some(attribs) = self.attributes.as_ref() {
            attribs.iter().for_each(|(k, v)| elem.push_attribute((k.as_str(), v.as_str())))
        }
        writer.write_event(Event::Start(elem))?;
        self.value.as_ref().map(|text| writer.write_event(Event::Text(BytesText::new(text.as_str()))));
        if let Some(children) = &self.children {
            for child in children {
                child.write_to(writer)?
            }
        }
        writer.write_event(Event::End(BytesEnd::new(self.name.as_str())))
    }
}


#[derive(Debug, Clone)]
pub(crate) struct Epg {
    pub attributes: Option<Rc<HashMap<String, String>>>,
    pub children: Vec<Rc<XmlTag>>,
}

impl Epg {
    pub(crate) fn write_to<W: std::io::Write>(&self, writer: &mut Writer<W>) -> Result<(), quick_xml::Error> {
        let mut elem = BytesStart::new("tv");
        if let Some(attribs) = self.attributes.as_ref() {
            attribs.iter().for_each(|(k, v)| elem.push_attribute((k.as_str(), v.as_str())))
        }
        writer.write_event(Event::Start(elem))?;
        for child in &self.children {
            child.write_to(writer)?
        }
        writer.write_event(Event::End(BytesEnd::new("tv")))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TVGuide {
    pub epg: XmlTag,
}

impl TVGuide {
    pub(crate) fn filter(&self, channel_ids: &HashSet<Rc<String>>) -> Option<Epg> {
        if !channel_ids.is_empty() {
            if let Some(epg_children) = &self.epg.children {
                let children: Vec<Rc<XmlTag>> = epg_children.iter().filter(|c| {
                    match c.name.as_str() {
                        "channel" => {
                            match c.get_attribute_value("id") {
                                None => false,
                                Some(val) => channel_ids.contains(val)
                            }
                        }
                        "programme" => {
                            match c.get_attribute_value("channel") {
                                None => false,
                                Some(val) => channel_ids.contains(val)
                            }
                        }
                        _ => false,
                    }
                }).cloned().collect();

                if !children.is_empty() {
                    return Some(Epg {
                        attributes: self.epg.attributes.clone(),
                        children,
                    });
                }
            }
        }
        None
    }
}

