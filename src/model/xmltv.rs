use std::collections::{HashMap};
use std::path::PathBuf;
use std::rc::Rc;

use quick_xml::{Error, Writer};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};

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
    pub attributes: Option<Rc<HashMap<String, String>>>,
    pub children: Option<Vec<Rc<XmlTag>>>,
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
}


#[derive(Debug, Clone)]
pub struct Epg {
    pub attributes: Option<Rc<HashMap<String, String>>>,
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
