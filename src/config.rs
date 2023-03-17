use path_absolutize::*;

use crate::filter::{Filter, get_filter, MockValueProcessor, PatternTemplate, prepare_templates, ValueProvider};
use crate::mapping::Mappings;
use crate::mapping::Mapping;
use crate::model::{ItemField, ProcessingOrder, SortOrder, TargetType, default_as_zero, default_as_empty_str, default_as_false, default_as_true};
use crate::utils;
use crate::utils::get_working_path;

fn default_as_frm() -> ProcessingOrder { ProcessingOrder::FRM }
fn default_as_default() -> String { String::from("default") }

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigSort {
    pub order: SortOrder,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigRename {
    pub field: ItemField,
    pub pattern: String,
    pub new_name: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub re: Option<regex::Regex>,
}

impl ConfigRename {
    pub(crate) fn prepare(&mut self) -> () {
        let re = regex::Regex::new(&self.pattern);
        if re.is_err() {
            println!("cant parse regex: {}", &self.pattern);
            std::process::exit(1);
        }
        self.re = Some(re.unwrap());
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigOptions {
    #[serde(default = "default_as_false")]
    pub ignore_logo: bool,
    #[serde(default = "default_as_false")]
    pub underscore_whitespace: bool,
    #[serde(default = "default_as_false")]
    pub cleanup: bool,
    #[serde(default = "default_as_false")]
    pub kodi_style: bool,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigTarget {
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    #[serde(default = "default_as_default")]
    pub name: String,
    pub filename: String,
    pub options: Option<ConfigOptions>,
    pub sort: Option<ConfigSort>,
    pub filter: String,
    #[serde(alias = "type")]
    pub output: Option<TargetType>,
    pub rename: Option<Vec<ConfigRename>>,
    pub mapping: Option<Vec<String>>,
    #[serde(default = "default_as_frm")]
    pub processing_order: ProcessingOrder,
    #[serde(skip_serializing, skip_deserializing)]
    pub _filter: Option<Filter>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _mapping: Option<Vec<Mapping>>,
}

impl ConfigTarget {
    pub(crate) fn prepare(&mut self, templates: Option<&Vec<PatternTemplate>>, verbose: bool) -> () {
        let fltr = get_filter(&self.filter, templates, verbose);
        if verbose { println!("Filter: {}", fltr)}
        self._filter = Some(fltr);
        match self.rename.as_mut() {
            Some(renames) => for r in renames {
                r.prepare();
            },
            _ => {}
        }
    }
    pub(crate) fn filter(&self, provider: &ValueProvider, verbose: bool) -> bool {
        let mut processor = MockValueProcessor{};
        return self._filter.as_ref().unwrap().filter(provider, &mut processor, verbose);
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigSources {
    pub input: ConfigInput,
    pub targets: Vec<ConfigTarget>,
}

impl ConfigSources {
    pub(crate) fn prepare(&mut self) {
        self.input.prepare();
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InputAffix {
    pub field: String,
    pub value: String
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigInput {
    pub url: String,
    #[serde(default = "default_as_true")]
    pub enabled: bool,
    #[serde(default = "default_as_empty_str")]
    pub persist: String,
    pub prefix: Option<InputAffix>,
    pub suffix: Option<InputAffix>,
}

impl ConfigInput {
    pub(crate) fn prepare(&mut self) {
        if self.persist.len() > 0 && self.persist.trim().is_empty() {
            self.persist = String::from("");
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigApi {
    pub host: String,
    pub port: u16,
    pub web_root: String,
}

impl ConfigApi {
    pub(crate) fn prepare(&mut self) {
        if self.web_root.is_empty() {
            self.web_root = String::from("./web");
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default = "default_as_zero")]
    pub threads: u8,
    pub api: ConfigApi,
    pub sources: Vec<ConfigSources>,
    pub working_dir: String,
    pub templates: Option<Vec<PatternTemplate>>,
}

impl Config {
    pub(crate) fn set_mappings(&mut self, mappings: Option<Mappings>) {
        match mappings {
            Some(mapping_list) => {
                for source in &mut self.sources {
                    for target in &mut source.targets {
                        match &target.mapping {
                            Some(mapping_ids) => {
                                let mut target_mappings = Vec::new();
                                for mapping_id in mapping_ids {
                                    let mapping = mapping_list.get_mapping(mapping_id);
                                    if mapping.is_some() {
                                        target_mappings.push(mapping.unwrap());
                                    }
                                }
                                target._mapping = if target_mappings.len() > 0 { Some(target_mappings) } else {  None };
                            },
                            _ => {},
                        }
                    }
                }
            },
            _ => {},
        }
    }

    pub(crate) fn prepare(&mut self, verbose: bool) {
        self.working_dir = get_working_path(&self.working_dir);
        self.api.prepare();
        self.prepare_api_web_root();
        match &mut self.templates {
          Some(templates) => self.templates = Some(prepare_templates(templates, verbose)),
            _ => {}
        };
        for source in &mut self.sources {
            source.prepare();
            for target in &mut source.targets {
                match &self.templates {
                    Some(templ) => target.prepare(Some(&templ), verbose),
                    _ => target.prepare(None, verbose)
                }
            }
        }
    }

    fn prepare_api_web_root(&mut self) {
        if !self.api.web_root.is_empty() {
            let wrpb = std::path::PathBuf::from(&self.api.web_root);
            if wrpb.is_relative() {
                let mut wrpb2 = std::path::PathBuf::from(&self.working_dir).join(&self.api.web_root);
                if !wrpb2.exists() {
                    wrpb2 = utils::get_exe_path().join(&self.api.web_root);
                }
                if !wrpb2.exists() {
                    let cwd = std::env::current_dir();
                    if cwd.is_ok() {
                        wrpb2 = cwd.unwrap().join(&self.api.web_root);
                    }
                }
                if wrpb2.exists() {
                    match wrpb2.absolutize() {
                        Ok(os) => self.api.web_root = String::from(os.to_str().unwrap()),
                        Err(e) => {
                            println!("failed to absolutize web_root {:?}", e);
                        }
                    }
                    // } else {
                    //     println!("web_root directory does not exists {:?}", wrpb2)
                }
            }
        }
    }
}

impl Clone for Config {
    fn clone(&self) -> Self {
        Config {
            threads: self.threads,
            api: self.api.clone(),
            sources: self.sources.clone(),
            working_dir: self.working_dir.clone(),
            templates: self.templates.clone(),
        }
    }
}

impl Clone for ConfigTarget {
    fn clone(&self) -> Self {
        ConfigTarget {
            enabled: self.enabled,
            name: self.name.clone(),
            filename: self.filename.clone(),
            options: self.options.as_ref().map(|o| o.clone()),
            sort: self.sort.as_ref().map(|s| s.clone()),
            filter: self.filter.clone(),
            output: self.output.clone(),
            rename: self.rename.clone(),
            mapping: self.mapping.clone(),
            processing_order: self.processing_order.clone(),
            _filter: self._filter.clone(),
            _mapping: self._mapping.clone(),
        }
    }
}

impl Clone for ConfigSources {
    fn clone(&self) -> Self {
        ConfigSources {
            input: self.input.clone(),
            targets: self.targets.clone(),
        }
    }
}

impl Clone for ConfigOptions {
    fn clone(&self) -> Self {
        ConfigOptions {
            ignore_logo: self.ignore_logo,
            underscore_whitespace: self.underscore_whitespace,
            cleanup: self.cleanup,
            kodi_style: self.kodi_style,
        }
    }
}

impl Clone for ConfigSort {
    fn clone(&self) -> Self {
        ConfigSort {
            order: self.order.clone(),
        }
    }
}

impl Clone for ConfigRename {
    fn clone(&self) -> Self {
        ConfigRename {
            field: self.field.clone(),
            pattern: self.pattern.clone(),
            new_name: self.new_name.clone(),
            re: None,
        }
    }
}
