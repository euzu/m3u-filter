use std::cell::RefCell;
use std::collections::HashMap;
use log::{debug, error};
use regex::Regex;
use crate::filter::{Filter, get_filter, PatternTemplate, prepare_templates, RegexWithCaptures, ValueProcessor};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::model_m3u::{FieldAccessor, PlaylistItem};
use crate::model::model_config::{ItemField, MAPPER_ATTRIBUTE_FIELDS, AFFIX_FIELDS,
                                 default_as_empty_str, default_as_false, default_as_empty_map, };
use crate::{handle_m3u_filter_error_result, valid_property};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct MappingTag {
    pub name: String,
    pub captures: Vec<String>,
    #[serde(default = "default_as_empty_str")]
    pub concat: String,
    #[serde(default = "default_as_empty_str")]
    pub prefix: String,
    #[serde(default = "default_as_empty_str")]
    pub suffix: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Mapper {
    pub filter: Option<String>,
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
    pub(crate) _pattern: Option<Filter>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _tags: Vec<MappingTag>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _tagre: Option<Regex>,
}

impl Mapper {
    pub fn prepare(&mut self, templates: Option<&Vec<PatternTemplate>>, tags: Option<&Vec<MappingTag>>) -> Result<(), M3uFilterError> {
        match get_filter(&self.pattern, templates) {
            Ok(pattern) => {
                self._pattern = Some(pattern);
                match &self.filter {
                    Some(flt) => {
                        match get_filter(flt, templates) {
                            Ok(filter) => self._filter = Some(filter),
                            Err(err) => return Err(err),
                        }
                    }
                    _ => self._filter = None
                }
                self._tags = match tags {
                    Some(list) => list.clone(),
                    _ => vec![]
                };
                self._tagre = Some(Regex::new("<tag:(.*?)>").unwrap());
                Ok(())
            }
            Err(err) => Err(err)
        }
    }
}

pub(crate) struct MappingValueProcessor<'a> {
    pub pli: RefCell<&'a PlaylistItem>,
    pub mapper: RefCell<&'a Mapper>,
}

impl MappingValueProcessor<'_> {
    fn get_property(&self, key: &str) -> Option<String> {
        self.pli.borrow().header.borrow().get_field(key).map(String::from)
    }

    fn set_property(&mut self, key: &str, value: &String) {
        if !self.pli.borrow().header.borrow_mut().set_field(key, value) {
            error!("Cant set unknown field {} to {}", key, value);
        }
        debug!("Property {} set to {}", key, value);
    }

    fn apply_attributes(&mut self) {
        let mapper = self.mapper.borrow();
        let attributes = &mapper.attributes;
        drop(mapper);
        for (key, value) in attributes {
            if valid_property!(key.as_str(), MAPPER_ATTRIBUTE_FIELDS) {
                self.set_property(key, value);
            }
        }
    }

    fn apply_tags(&mut self, value: &String, captures: &HashMap<&String, &str>) -> Option<String> {
        let mut new_value = String::from(value);
        let tag_captures = self.mapper.borrow()._tagre.as_ref().unwrap().captures_iter(value)
            .filter(|caps| caps.len() > 1)
            .filter_map(|caps| caps.get(1))
            .map(|caps| caps.as_str())
            .collect::<Vec<&str>>();

        for tag_capture in tag_captures {
            for mapping_tag in &self.mapper.borrow()._tags {
                if mapping_tag.name.eq(tag_capture) {
                    // we have the right tag, now get all captured values
                    let mut captured_tag_values: Vec<&str> = Vec::new();
                    for cap in &mapping_tag.captures {
                        match captures.get(&cap) {
                            Some(cap_value) => captured_tag_values.push(cap_value),
                            _ => {
                                debug!("Cant find any tag match for {}", tag_capture);
                                return None;
                            }
                        }
                    }
                    if !captured_tag_values.is_empty() {
                        let captured_text = captured_tag_values.join(&mapping_tag.concat);
                        let replacement = if !captured_text.trim().is_empty() {
                            // Now we have all our captured values, lets create the tag
                            format!("{}{}{}", &mapping_tag.prefix, captured_text, &mapping_tag.suffix)
                        } else {
                            // nothing found so replace tag with empty string
                            String::from("")
                        };
                        new_value = new_value.replace(format!("<tag:{}>", mapping_tag.name).as_str(), replacement.as_str());
                    }
                }
            }
        }
        Some(new_value)
    }

    fn apply_suffix(&mut self, captures: &HashMap<&String, &str>) {
        let mapper = self.mapper.borrow();
        let suffix = &mapper.suffix;
        drop(mapper);

        for (key, value) in suffix {
            if valid_property!(key.as_str(), AFFIX_FIELDS) {
                if let Some(suffix) = self.apply_tags(value, captures) {
                    if let Some(old_value) = self.get_property(key) {
                        let new_value = format!("{}{}", &old_value, suffix);
                        self.set_property(key, &new_value);
                    }
                }
            }
        }
    }

    fn apply_prefix(&mut self, captures: &HashMap<&String, &str>) {
        let mapper = self.mapper.borrow();
        let prefix = &mapper.prefix;
        drop(mapper);
        for (key, value) in prefix {
            if valid_property!(key.as_str(), AFFIX_FIELDS) {
                if let Some(prefix) = self.apply_tags(value, captures) {
                    if let Some(old_value) = self.get_property(key) {
                        let new_value = format!("{}{}", prefix, &old_value);
                        self.set_property(key, &new_value);
                    }
                }
            }
        }
    }

    fn apply_assignments(&mut self) {
        let mapper = self.mapper.borrow();
        let assignments = &mapper.assignments;
        drop(mapper);
        for (key, value) in assignments {
            if valid_property!(key.as_str(), MAPPER_ATTRIBUTE_FIELDS) &&
                valid_property!(value.as_str(), MAPPER_ATTRIBUTE_FIELDS) {
                if let Some(prop_value) = self.get_property(value) {
                    self.set_property(key, &prop_value);
                }
            }
        }
    }
}

impl ValueProcessor for MappingValueProcessor<'_> {
    fn process<'a>(&mut self, _: &ItemField, value: &str, rewc: &RegexWithCaptures) -> bool {
        let mut captured_values = HashMap::new();
        if !rewc.captures.is_empty() {
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
                        debug!("match {}: {}", capture_name, capture_value);
                        captured_values.insert(capture_name, capture_value);
                    }
                );
        }
        let _ = &MappingValueProcessor::<'_>::apply_attributes(self);
        let _ = &MappingValueProcessor::<'_>::apply_suffix(self, &captured_values);
        let _ = &MappingValueProcessor::<'_>::apply_prefix(self, &captured_values);
        let _ = &MappingValueProcessor::<'_>::apply_assignments(self);
        true
    }
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Mapping {
    pub id: String,
    #[serde(default = "default_as_false")]
    pub match_as_ascii: bool,
    pub mapper: Vec<Mapper>,
}


impl Mapping {
    pub fn prepare(&mut self, templates: Option<&Vec<PatternTemplate>>,
                   tags: Option<&Vec<MappingTag>>) -> Result<(), M3uFilterError> {
        for mapper in &mut self.mapper {
            handle_m3u_filter_error_result!(M3uFilterErrorKind::Info, mapper.prepare(templates, tags));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct MappingDefinition {
    pub templates: Option<Vec<PatternTemplate>>,
    pub tags: Option<Vec<MappingTag>>,
    pub mapping: Vec<Mapping>,
}

impl MappingDefinition {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        if let Some(templates) = &mut self.templates {
            match prepare_templates(templates) {
                Ok(tmplts) => {
                    self.templates = Some(tmplts);
                }
                Err(err) => return Err(err),
            }

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
            handle_m3u_filter_error_result!(M3uFilterErrorKind::Info, mapping.prepare(template_list, tag_list));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Mappings {
    pub mappings: MappingDefinition,
}

impl Mappings {
    pub fn prepare(&mut self) -> Result<(), M3uFilterError> {
        self.mappings.prepare()
    }

    pub fn get_mapping(&self, mapping_id: &String) -> Option<Mapping> {
        for mapping in &self.mappings.mapping {
            if mapping.id.eq(mapping_id) {
                return Some(mapping.clone());
            }
        }
        None
    }
}

