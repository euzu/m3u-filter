use std::collections::HashMap;
use crate::filter::{Filter, get_filter, PatternTemplate, RegexWithCaptures, ValueProcessor};
use crate::m3u::PlaylistItem;
use crate::model::ItemField;

fn default_as_false() -> bool { false }
fn default_as_empty_str() -> String { String::from("") }
fn default_as_empty_map() -> HashMap<String, String> { HashMap::new() }

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mapper {
    pub pattern: String,
    #[serde(default = "default_as_empty_map")]
    attributes: HashMap<String, String>,
    #[serde(default = "default_as_empty_map")]
    suffix: HashMap<String, String>,
    #[serde(default = "default_as_empty_map")]
    prefix: HashMap<String, String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _filter: Option<Filter>,
}

impl Mapper {
    pub(crate) fn prepare<'a>(&mut self, templates: Option<&Vec<PatternTemplate>>) -> () {
        self._filter = Some(get_filter(&self.pattern, templates));
    }
}

impl Clone for Mapper {
    fn clone(&self) -> Self {
        Mapper {
            pattern: self.pattern.clone(),
            attributes: self.attributes.clone(),
            suffix: self.suffix.clone(),
            prefix: self.prefix.clone(),
            _filter: self._filter.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MappingTag {
    pub captures: Vec<String>,
    #[serde(default = "default_as_empty_str")]
    pub concat: String,
    #[serde(default = "default_as_empty_str")]
    pub prefix: String,
    #[serde(default = "default_as_empty_str")]
    pub suffix: String,
}

impl Clone for MappingTag {
    fn clone(&self) -> Self {
        MappingTag {
            captures: self.captures.clone(),
            concat: self.prefix.clone(),
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
        }
    }
}

pub struct MappingValueProcessor<'a> {
    pub(crate) pli: &'a mut PlaylistItem,
    pub(crate) mapping_tag: &'a MappingTag,
    pub(crate) mapper: &'a Mapper,
}

impl ValueProcessor for MappingValueProcessor<'_> {
    fn process<'a>(&mut self, _: &ItemField, value: &str, rewc: &RegexWithCaptures, verbose: bool) -> bool {
        let mut tag = "".to_string();
        //let mut captured_values = HashMap::new();
        if rewc.captures.len() > 0 {
            let captures_opt = rewc.re.captures(value);
            if captures_opt.is_some() {
                let captures = captures_opt.unwrap();
                for cname in &rewc.captures {
                    let match_opt = captures.name(cname.as_str());
                    let repl = if match_opt.is_some() {
                        match_opt.map_or("", |m| m.as_str())
                    } else {
                        ""
                    };
                    //captured_values.insert(cname, repl);
                    for c in &self.mapping_tag.captures {
                        if c.eq(cname) {
                            if tag.is_empty() {
                                tag = String::from(repl);
                            } else {
                                tag = format!("{}{}{}", tag,  &self.mapping_tag.concat, String::from(repl))
                            }
                        }
                    }
                }
            }
        }

        for (key, value) in &self.mapper.attributes {
            let new_value = String::from(value);
            if verbose { println!("Attribute {} set to {}", key, new_value)}
            if "name".eq(key) {
                self.pli.header.name = String::from(new_value);
            } else if "group".eq(key) {
                self.pli.header.group = String::from(new_value);
            } else if "title".eq(key) {
                self.pli.header.title = String::from(new_value);
            } else if "logo".eq(key) {
                self.pli.header.logo = String::from(new_value);
            } else if "id".eq(key) {
                self.pli.header.id = String::from(new_value);
            } else if "chno".eq(key) {
                self.pli.header.chno = String::from(new_value);
            } else {
                println!("Unknown field {} in attributes", new_value);
            }
        }

        if !tag.is_empty() {
            tag = format!("{}{}{}", &self.mapping_tag.prefix, tag, &self.mapping_tag.suffix)
        }

        for (key, value) in &self.mapper.suffix {
            let mut new_value = String::from(value);
            if value.contains("<tag>") {
                if tag.is_empty() {
                    if verbose { println!("No tag exists, skipping tag suffix for {}", key) }
                    continue;
                }
                new_value = String::from(value.replace("<tag>", &tag));
            }

            if verbose { println!("Adding suffix to {}: {}", key, new_value)}
            if "name".eq(key) {
                self.pli.header.name = format!("{}{}", &self.pli.header.name,  new_value).to_string();
            } else if "group".eq(key) {
                self.pli.header.group = format!("{}{}", &self.pli.header.group,  new_value).to_string();
            } else if "title".eq(key) {
                self.pli.header.title = format!("{}{}", &self.pli.header.title,  new_value).to_string();
            } else {
                println!("Unknown field {} in suffix", key);
            }
        }

        for (key, value) in &self.mapper.prefix {
            let mut new_value = String::from(value);
            if value.contains("<tag>") {
                if tag.is_empty() {
                    if verbose { println!("No tag exists, skipping tag prefix for {}", key) }
                    continue;
                }
                new_value = String::from(value.replace("<tag>", &tag));
            }

            if verbose { println!("Adding prefix to {}: {}", key, new_value)}
            if "name".eq(key) {
                self.pli.header.name = format!("{}{}", new_value, &self.pli.header.name).to_string();
            } else if "group".eq(key) {
                self.pli.header.group = format!("{}{}", new_value, &self.pli.header.group).to_string();
            } else if "title".eq(key) {
                self.pli.header.title = format!("{}{}",  new_value, &self.pli.header.title).to_string();
            } else {
                println!("Unknown field {} in prefix", key);
            }
        }
        true
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mapping {
    pub id: String,
    pub tag: Option<MappingTag>,
    #[serde(default = "default_as_false")]
    pub match_as_ascii: bool,
    pub mapper: Vec<Mapper>,
}

impl Clone for Mapping {
    fn clone(&self) -> Self {
        Mapping {
            id: self.id.clone(),
            tag: self.tag.clone(),
            match_as_ascii: self.match_as_ascii,
            mapper: self.mapper.clone(),
        }
    }
}

impl Mapping {
    pub(crate) fn prepare(&mut self, templates: Option<&Vec<PatternTemplate>>) -> () {
        for mapper in &mut self.mapper {
            match templates {
                Some(templ) => mapper.prepare(Some(templ)),
                _ => mapper.prepare(None)
            }
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MappingDefinition {
    pub templates: Option<Vec<PatternTemplate>>,
    pub mapping: Vec<Mapping>
}

impl MappingDefinition {
    pub(crate) fn prepare(&mut self) {
        for mapping in &mut self.mapping {
            match &self.templates {
                Some(templ) => mapping.prepare(Some(&templ)),
                _ => mapping.prepare(None)
            }
        }
    }
}

impl Clone for MappingDefinition {
    fn clone(&self) -> Self {
        MappingDefinition {
            templates: self.templates.clone(),
            mapping: self.mapping.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mappings {
    pub mappings: MappingDefinition
}

impl Mappings {
    pub(crate) fn prepare(&mut self) {
        self.mappings.prepare();
    }

    pub(crate) fn get_mapping(&self, mapping_id: &String) -> Option<Mapping> {
        for mapping in &self.mappings.mapping {
            if mapping.id.eq(mapping_id) {
                return Some(mapping.clone())
            }
        }
        None
    }
}

impl Clone for Mappings {
    fn clone(&self) -> Self {
        Mappings {
            mappings: self.mappings.clone(),
        }
    }
}
