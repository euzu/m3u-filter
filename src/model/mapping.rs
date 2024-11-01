use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Display;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use enum_iterator::Sequence;
use log::{debug, error};
use regex::Regex;

use crate::{create_m3u_filter_error_result, handle_m3u_filter_error_result, valid_property};
use crate::filter::{apply_templates_to_pattern, Filter, get_filter, PatternTemplate, prepare_templates, RegexWithCaptures, ValueProcessor};
use crate::m3u_filter_error::{M3uFilterError, M3uFilterErrorKind};
use crate::model::config::{AFFIX_FIELDS, COUNTER_FIELDS, ItemField, MAPPER_ATTRIBUTE_FIELDS};
use crate::model::playlist::{FieldAccessor, PlaylistItem};
use crate::utils::string_utils::Capitalize;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct MappingTag {
    pub name: String,
    pub captures: Vec<String>,
    #[serde(default)]
    pub concat: String,
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub suffix: String,
}


#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq)]
pub(crate) enum CounterModifier {
    #[serde(rename = "assign")]
    Assign,
    #[serde(rename = "suffix")]
    Suffix,
    #[serde(rename = "prefix")]
    Prefix,
}

impl Default for CounterModifier {
    fn default() -> Self {
        Self::Assign
    }
}

impl CounterModifier {
    const ASSIGN: &'static str = "assign";
    const SUFFIX: &'static str = "suffix";
    const PREFIX: &'static str = "prefix";
}

impl Display for CounterModifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Assign => Self::ASSIGN,
            Self::Suffix => Self::SUFFIX,
            Self::Prefix => Self::PREFIX,
        })
    }
}

impl FromStr for CounterModifier {
    type Err = M3uFilterError;

    fn from_str(s: &str) -> Result<Self, M3uFilterError> {
        if s.eq("assign") {
            Ok(Self::Assign)
        } else if s.eq("suffix") {
            Ok(Self::Suffix)
        } else if s.eq("prefix") {
            Ok(Self::Prefix)
        } else {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown CounterModifier: {}", s)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct MappingCounterDefinition {
    pub filter: String,
    pub field: String,
    #[serde(default)]
    pub concat: String,
    #[serde(default)]
    pub modifier: CounterModifier,
    #[serde(default)]
    pub value: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct MappingCounter {
    pub filter: Filter,
    pub field: String,
    pub concat: String,
    pub modifier: CounterModifier,
    pub value: Arc<Mutex<u32>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence, PartialEq)]
pub(crate) enum TransformModifier {
    #[serde(rename = "lowercase")]
    Lowercase,
    #[serde(rename = "uppercase")]
    Uppercase,
    #[serde(rename = "capitalize")]
    Capitalize,
}

impl Default for TransformModifier {
    fn default() -> Self {
        Self::Lowercase
    }
}

impl TransformModifier {
    const LOWERCASE: &'static str = "lowercase";
    const UPPERCASE: &'static str = "uppercase";
    const CAPITALIZE: &'static str = "capitalize";
}

impl Display for TransformModifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Lowercase => Self::LOWERCASE,
            Self::Uppercase => Self::UPPERCASE,
            Self::Capitalize => Self::CAPITALIZE,
        })
    }
}

impl FromStr for TransformModifier {
    type Err = M3uFilterError;

    fn from_str(s: &str) -> Result<Self, M3uFilterError> {
        if s.eq("lowercase") {
            Ok(Self::Lowercase)
        } else if s.eq("uppercase") {
            Ok(Self::Uppercase)
        } else if s.eq("capitalize") {
            Ok(Self::Capitalize)
        } else {
            create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "Unknown TransformModifier: {}", s)
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct MapperTransform {
    pub field: String,
    pub modifier: TransformModifier,
    pub pattern: Option<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_pattern: Option<Regex>,
}

impl MapperTransform {
    pub fn prepare(&mut self, templates: Option<&Vec<PatternTemplate>>) -> Result<(), M3uFilterError> {
        match &self.pattern {
            None => self.t_pattern = None,
            Some(pattern) => {
                let mut new_pattern = pattern.to_string();
                match templates {
                    None => {}
                    Some(template_list) => {
                        new_pattern = apply_templates_to_pattern(pattern, template_list);
                    }
                }
                let re = Regex::new(&new_pattern);
                if re.is_err() {
                    return create_m3u_filter_error_result!(M3uFilterErrorKind::Info, "cant parse regex: {}", new_pattern);
                }
                self.t_pattern = Some(re.unwrap());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct Mapper {
    pub filter: Option<String>,
    pub pattern: String,
    #[serde(default)]
    attributes: HashMap<String, String>,
    #[serde(default)]
    suffix: HashMap<String, String>,
    #[serde(default)]
    prefix: HashMap<String, String>,
    #[serde(default)]
    assignments: HashMap<String, String>,
    #[serde(default)]
    transform: Option<Vec<MapperTransform>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) t_filter: Option<Filter>,
    #[serde(skip_serializing, skip_deserializing)]
    pub(crate) t_pattern: Option<Filter>,
    #[serde(skip_serializing, skip_deserializing)]
    t_tags: Vec<MappingTag>,
    #[serde(skip_serializing, skip_deserializing)]
    t_tagre: Option<Regex>,
    #[serde(skip_serializing, skip_deserializing)]
    t_attre: Option<Regex>,
}

impl Mapper {
    pub fn prepare(&mut self, templates: Option<&Vec<PatternTemplate>>, tags: Option<&Vec<MappingTag>>) -> Result<(), M3uFilterError> {
        for key in self.attributes.keys() {
            if !valid_property!(key.as_str(), MAPPER_ATTRIBUTE_FIELDS) {
                return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Invalid mapper attribute field {key}")));
            }
        }
        for key in self.suffix.keys() {
            if !valid_property!(key.as_str(), AFFIX_FIELDS) {
                return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Invalid mapper suffix field {key}")));
            }
        }
        for key in self.prefix.keys() {
            if !valid_property!(key.as_str(), AFFIX_FIELDS) {
                return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Invalid mapper prefix field {key}")));
            }
        }
        for (key, value) in &self.assignments {
            if !valid_property!(key.as_str(), MAPPER_ATTRIBUTE_FIELDS) {
                return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Invalid mapper assignment field {key}")));
            }
            if !valid_property!(value.as_str(), MAPPER_ATTRIBUTE_FIELDS) {
                return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Invalid mapper assignment field {value}")));
            }
        }

        match &mut self.transform {
            None => {}
            Some(transforms) => {
                for t in transforms {
                    let field = t.field.as_str();
                    if !valid_property!(field, AFFIX_FIELDS) {
                        return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Invalid mapper transform field {field}")));
                    }
                    t.prepare(templates)?;
                }
            }
        }

        match get_filter(&self.pattern, templates) {
            Ok(pattern) => {
                self.t_pattern = Some(pattern);
                match &self.filter {
                    Some(flt) => {
                        match get_filter(flt, templates) {
                            Ok(filter) => self.t_filter = Some(filter),
                            Err(err) => return Err(err),
                        }
                    }
                    _ => self.t_filter = None
                }
                self.t_tags = match tags {
                    Some(list) => list.clone(),
                    _ => vec![]
                };
                self.t_tagre = Some(Regex::new("<tag:(.*?)>").unwrap());
                self.t_attre = Some(Regex::new("<(.*?)>").unwrap());
                Ok(())
            }
            Err(err) => Err(err)
        }
    }
}

pub(crate) struct MappingValueProcessor<'a> {
    pub pli: RefCell<&'a PlaylistItem>,
    pub mapper: &'a Mapper,
}

impl MappingValueProcessor<'_> {
    fn get_property(&self, key: &str) -> Option<Rc<String>> {
        self.pli.borrow().header.borrow().get_field(key)
    }

    fn set_property(&mut self, key: &str, value: &str) {
        if !self.pli.borrow().header.borrow_mut().set_field(key, value) {
            error!("Cant set unknown field {} to {}", key, value);
        }
        debug!("Property {} set to {}", key, value);
    }

    fn apply_attributes(&mut self, captured_names: &HashMap<&str, &str>) {
        let mapper = self.mapper;
        let attr_re = &mapper.t_attre.as_ref().unwrap();
        let attributes = &mapper.attributes;
        for (key, value) in attributes {
            if value.contains('<') { // possible replacement
                let replaced = attr_re.replace_all(value, |captures: &regex::Captures| {
                    let capture_name = &captures[1];
                    (*captured_names.get(&capture_name).unwrap_or(&&captures[0])).to_string()
                });
                self.set_property(key, &replaced);
            } else {
                self.set_property(key, value);
            }
        }
    }

    fn apply_tags(&mut self, value: &String, captures: &HashMap<&str, &str>) -> Option<String> {
        let mut new_value = String::from(value);
        let tag_captures = self.mapper.t_tagre.as_ref().unwrap().captures_iter(value)
            .filter(|caps| caps.len() > 1)
            .filter_map(|caps| caps.get(1))
            .map(|caps| caps.as_str())
            .collect::<Vec<&str>>();

        for tag_capture in tag_captures {
            for mapping_tag in &self.mapper.t_tags {
                if mapping_tag.name.eq(tag_capture) {
                    // we have the right tag, now get all captured values
                    let mut captured_tag_values: Vec<&str> = Vec::new();
                    for cap in &mapping_tag.captures {
                        if let Some(cap_value) = captures.get(cap.as_str()) {
                            captured_tag_values.push(cap_value);
                        } else {
                            debug!("Cant find any tag match for {}", tag_capture);
                            return None;
                        }
                    }
                    if !captured_tag_values.is_empty() {
                        let captured_text = captured_tag_values.join(&mapping_tag.concat);
                        let replacement = if captured_text.trim().is_empty() {
                            // nothing found so replace tag with empty string
                            String::new()
                        } else {
                            // Now we have all our captured values, lets create the tag
                            format!("{}{captured_text}{}", &mapping_tag.prefix, &mapping_tag.suffix)
                        };
                        new_value = new_value.replace(format!("<tag:{}>", mapping_tag.name).as_str(), replacement.as_str());
                    }
                }
            }
        }
        Some(new_value)
    }

    fn apply_suffix(&mut self, captures: &HashMap<&str, &str>) {
        let mapper = self.mapper;
        let suffix = &mapper.suffix;

        for (key, value) in suffix {
            if let Some(suffix) = self.apply_tags(value, captures) {
                if let Some(old_value) = self.get_property(key) {
                    let new_value = format!("{}{}", &old_value, suffix);
                    self.set_property(key, &new_value);
                }
            }
        }
    }

    fn apply_prefix(&mut self, captures: &HashMap<&str, &str>) {
        let mapper = self.mapper;
        let prefix = &mapper.prefix;
        for (key, value) in prefix {
            if let Some(prefix) = self.apply_tags(value, captures) {
                if let Some(old_value) = self.get_property(key) {
                    let new_value = format!("{}{}", prefix, &old_value);
                    self.set_property(key, &new_value);
                }
            }
        }
    }

    fn apply_assignments(&mut self) {
        let mapper = self.mapper;
        let assignments = &mapper.assignments;
        for (key, value) in assignments {
            if let Some(prop_value) = self.get_property(value) {
                self.set_property(key, &prop_value);
            }
        }
    }

    fn apply_transform_modifier(modifier: &TransformModifier, value: &str) -> String {
        match modifier {
            TransformModifier::Uppercase => value.to_uppercase(),
            TransformModifier::Lowercase => value.to_lowercase(),
            TransformModifier::Capitalize => value.capitalize(),
        }
    }

    fn apply_transform(&mut self) {
        let mapper = self.mapper;
        match &mapper.transform {
            None => {}
            Some(transform_list) => {
                for transform in transform_list {
                    if let Some(prop_value) = self.get_property(&transform.field) {
                        let value = if let Some(regex) = &transform.t_pattern {
                            regex.replace_all(&prop_value, |caps: &regex::Captures| {
                                Self::apply_transform_modifier(&transform.modifier, &caps[0])
                            })
                        } else {
                            Cow::from(Self::apply_transform_modifier(&transform.modifier, prop_value.as_str()))
                        };
                        self.set_property(&transform.field, &value);
                    }
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
                    captured_values.insert(capture_name.as_str(), capture_value);
                }
                );
        }
        MappingValueProcessor::<'_>::apply_attributes(self, &captured_values);
        MappingValueProcessor::<'_>::apply_suffix(self, &captured_values);
        MappingValueProcessor::<'_>::apply_prefix(self, &captured_values);
        MappingValueProcessor::<'_>::apply_assignments(self);
        MappingValueProcessor::<'_>::apply_transform(self);
        true
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct Mapping {
    pub id: String,
    #[serde(default)]
    pub match_as_ascii: bool,
    pub mapper: Vec<Mapper>,
    pub counter: Option<Vec<MappingCounterDefinition>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub t_counter: Option<Vec<MappingCounter>>,

}

impl Mapping {
    pub fn prepare(&mut self, templates: Option<&Vec<PatternTemplate>>,
                   tags: Option<&Vec<MappingTag>>) -> Result<(), M3uFilterError> {
        for mapper in &mut self.mapper {
            handle_m3u_filter_error_result!(M3uFilterErrorKind::Info, mapper.prepare(templates, tags));
        }

        if let Some(counter_def_list) = &self.counter {
            let mut counters = vec![];
            for def in counter_def_list {
                if !valid_property!(def.field.as_str(), COUNTER_FIELDS) {
                    return Err(M3uFilterError::new(M3uFilterErrorKind::Info, format!("Invalid counter field {}", def.field)));
                }
                match get_filter(&def.filter, templates) {
                    Ok(flt) => {
                        counters.push(MappingCounter {
                            filter: flt,
                            field: def.field.clone(),
                            concat: def.concat.clone(),
                            modifier: def.modifier.clone(),
                            value: Arc::new(Mutex::new(def.value)),
                        });
                    }
                    Err(e) => return Err(M3uFilterError::new(M3uFilterErrorKind::Info, e.to_string()))
                }
            }
            self.t_counter = Some(counters);
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
