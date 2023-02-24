use std::collections::HashMap;
use regex::Regex;
use crate::filter::{Filter, get_filter, PatternTemplate, prepare_templates, RegexWithCaptures, ValueProcessor};
use crate::m3u::{FieldAccessor, PlaylistItem};
use crate::model::{ItemField, MAPPER_ATTRIBUTE_FIELDS, MAPPER_PREFIX_SUFFIX_FIELDS};

fn default_as_false() -> bool { false }
fn default_as_empty_str() -> String { String::from("") }
fn default_as_empty_map() -> HashMap<String, String> { HashMap::new() }

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MappingTag {
    pub name: String,
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
            name: self.name.clone(),
            captures: self.captures.clone(),
            concat: self.concat.clone(),
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mapper {
    pub pattern: String,
    #[serde(default = "default_as_empty_map")]
    attributes: HashMap<String, String>,
    #[serde(default = "default_as_empty_map")]
    suffix: HashMap<String, String>,
    #[serde(default = "default_as_empty_map")]
    prefix: HashMap<String, String>,
    #[serde(default = "default_as_empty_map")]
    assignments: HashMap<String, String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) _filter: Option<Filter>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _tags: Vec<MappingTag>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _tagre: Option<Regex>,
}

impl Mapper {
    pub(crate) fn prepare<'a>(&mut self, templates: Option<&Vec<PatternTemplate>>, tags: Option<&Vec<MappingTag>>, verbose: bool) -> () {
        self._filter = Some(get_filter(&self.pattern, templates, verbose));
        self._tags = match tags {
            Some(list) => list.clone(),
            _ => vec![]
        };
        self._tagre = Some(Regex::new("<tag:(.*?)>").unwrap())
    }
}

impl Clone for Mapper {
    fn clone(&self) -> Self {
        Mapper {
            pattern: self.pattern.clone(),
            attributes: self.attributes.clone(),
            suffix: self.suffix.clone(),
            prefix: self.prefix.clone(),
            assignments: self.assignments.clone(),
            _filter: self._filter.clone(),
            _tags: self._tags.clone(),
            _tagre: self._tagre.clone(),
        }
    }
}

macro_rules! valid_property {
  ($key:expr, $array:expr) => {{
        $array.contains(&$key)
    }};
}

pub struct MappingValueProcessor<'a> {
    pub(crate) pli: &'a mut PlaylistItem,
    pub(crate) mapper: &'a Mapper,
}

impl MappingValueProcessor<'_> {
    fn get_property(&self, key: &str) -> Option<String> {
        match self.pli.header.get_field(key) {
            Some(value) => Some(String::from(value)),
            _=> None
        }
    }

    fn set_property(&mut self, key: &str, value: &String, verbose: bool) {
        if !self.pli.header.set_field(key, value) {
            println!("Cant set unknown field {} to {}", key, value);
        }
        if verbose { println!("Property {} set to {}", key, value) }
    }

    fn apply_attributes(&mut self, verbose: bool) {
        for (key, value) in &self.mapper.attributes {
            if valid_property!(key.as_str(), MAPPER_ATTRIBUTE_FIELDS) {
                self.set_property(key, value, verbose);
            }
        }
    }

    fn apply_tags(&mut self, value: &String, captures: &HashMap<&String, &str>, verbose: bool) -> Option<String> {
        let mut new_value = String::from(value);
        let tag_captures = self.mapper._tagre.as_ref().unwrap().captures_iter(value)
            .filter(|caps| caps.len() > 1)
            .filter_map(|caps| caps.get(1))
            .map(|caps| caps.as_str())
            .collect::<Vec<&str>>();

        for tag_capture in tag_captures {
            for mapping_tag in &self.mapper._tags {
                if mapping_tag.name.eq(tag_capture) {
                    // we have the right tag, now get all captured values
                    let mut captured_tag_values : Vec<&str> = Vec::new();
                    for cap in &mapping_tag.captures {
                        match captures.get(&cap) {
                            Some(cap_value) => captured_tag_values.push(cap_value),
                            _ => {
                                if verbose { println!("Cant find any tag match for {}", tag_capture) }
                                return None
                            }
                        }
                    }
                    if captured_tag_values.len() > 0 {
                        let captured_text = captured_tag_values.join(&mapping_tag.concat);
                        if captured_text.trim().len() > 0 {
                            // Now we have all our captured values, lets create the tag
                            new_value = new_value.replace(
                                format!("<tag:{}>", mapping_tag.name).as_str(),
                                format!("{}{}{}", &mapping_tag.prefix, captured_text, &mapping_tag.suffix).as_str());
                        }
                    }
                }
            }
        }
        Some(new_value)
    }

    fn apply_suffix(&mut self, captures: &HashMap<&String, &str>, verbose: bool) {
        for (key, value) in &self.mapper.suffix {
            if valid_property!(key.as_str(), MAPPER_PREFIX_SUFFIX_FIELDS) {
                match self.apply_tags(value, captures, verbose) {
                    Some(suffix) => {
                        match self.get_property(key) {
                            Some(old_value) => {
                                let new_value = format!("{}{}", &old_value, suffix);
                                self.set_property(key, &new_value, verbose);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn apply_prefix(&mut self, captures: &HashMap<&String, &str>, verbose: bool) {
        for (key, value) in &self.mapper.prefix {
            if valid_property!(key.as_str(), MAPPER_PREFIX_SUFFIX_FIELDS) {
                match self.apply_tags(value, captures, verbose) {
                    Some(prefix) => {
                        match self.get_property(key) {
                            Some(old_value) => {
                                let new_value = format!("{}{}", prefix, &old_value);
                                self.set_property(key, &new_value, verbose);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn apply_assignments(&mut self, verbose: bool) {
        for (key, value) in &self.mapper.assignments {
            if valid_property!(key.as_str(), MAPPER_ATTRIBUTE_FIELDS) &&
                valid_property!(value.as_str(), MAPPER_ATTRIBUTE_FIELDS) {
                match self.get_property(value) {
                    Some(prop_value) => {
                        self.set_property(key, &prop_value, verbose);
                    },
                    _ => {}
                }
            }
        }
    }

}

impl ValueProcessor for MappingValueProcessor<'_> {
    fn process<'a>(&mut self, _: &ItemField, value: &str, rewc: &RegexWithCaptures, verbose: bool) -> bool {
        let mut captured_values = HashMap::new();
        if rewc.captures.len() > 0 {
            rewc.re.captures_iter(value)
                .filter(|caps| caps.len() > 1)
                .for_each(|captures|
                    for capture_name in &rewc.captures {
                        let match_opt = captures.name(capture_name.as_str());
                        let capture_value = if match_opt.is_some() {
                            match_opt.map_or("", |m| m.as_str())
                        } else {
                            ""
                        };
                        if verbose { println!("match {}: {}", capture_name, capture_value); }
                        captured_values.insert(capture_name, capture_value);
                    }
                );
        }
        let _ = &MappingValueProcessor::<'_>::apply_attributes(self, verbose);
        let _ = &MappingValueProcessor::<'_>::apply_suffix(self, &captured_values, verbose);
        let _ = &MappingValueProcessor::<'_>::apply_prefix(self, &captured_values, verbose);
        let _ = &MappingValueProcessor::<'_>::apply_assignments(self, verbose);
        true
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mapping {
    pub id: String,
    #[serde(default = "default_as_false")]
    pub match_as_ascii: bool,
    pub mapper: Vec<Mapper>,
}

impl Clone for Mapping {
    fn clone(&self) -> Self {
        Mapping {
            id: self.id.clone(),
            match_as_ascii: self.match_as_ascii,
            mapper: self.mapper.clone(),
        }
    }
}

impl Mapping {
    pub(crate) fn prepare(&mut self, templates: Option<&Vec<PatternTemplate>>,
                          tags: Option<&Vec<MappingTag>>, verbose: bool) -> () {
        for mapper in &mut self.mapper {
            let template_list = match templates {
                Some(templ) => Some(templ),
                _ => None
            };
            let tag_list = match tags {
                Some(t) => Some(t),
                _ => None
            };
            mapper.prepare(template_list, tag_list, verbose);
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MappingDefinition {
    pub templates: Option<Vec<PatternTemplate>>,
    pub tags: Option<Vec<MappingTag>>,
    pub mapping: Vec<Mapping>,
}

impl MappingDefinition {
    pub(crate) fn prepare(&mut self, verbose: bool) {
        match &mut self.templates {
            Some(templates) =>self.templates = Some(prepare_templates(templates, verbose)),
            _ => {}
        };
        for mapping in &mut self.mapping {
            let template_list = match &self.templates {
                Some(templ) => Some(templ),
                _ => None
            };
            let tag_list = match &self.tags {
                Some(t) => Some(t),
                _ => None
            };
            mapping.prepare(template_list, tag_list, verbose);
        }
    }
}

impl Clone for MappingDefinition {
    fn clone(&self) -> Self {
        MappingDefinition {
            templates: self.templates.clone(),
            tags: self.tags.clone(),
            mapping: self.mapping.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mappings {
    pub mappings: MappingDefinition,
}

impl Mappings {
    pub(crate) fn prepare(&mut self, verbose: bool) {
        self.mappings.prepare(verbose);
    }

    pub(crate) fn get_mapping(&self, mapping_id: &String) -> Option<Mapping> {
        for mapping in &self.mappings.mapping {
            if mapping.id.eq(mapping_id) {
                return Some(mapping.clone());
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
