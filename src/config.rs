use path_absolutize::*;

use crate::filter::{Filter, get_filter, ValueProvider};
use crate::mapping::Mappings;
use crate::mapping::Mapping;
use crate::model::{ItemField, ProcessingOrder, SortOrder, TargetType};
use crate::utils;
use crate::utils::get_working_path;


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

fn default_as_false() -> bool {
    false
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


fn default_as_frm() -> ProcessingOrder {
    ProcessingOrder::Frm
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigTarget {
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
    pub(crate) fn prepare(&mut self) -> () {
        self._filter = Some(get_filter(&self.filter));
        match self.rename.as_mut() {
            Some(renames) => for r in renames {
                r.prepare();
            },
            _ => {}
        }
    }
    pub(crate) fn filter(&self, provider: &ValueProvider) -> bool {
        return self._filter.as_ref().unwrap().filter(provider);
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
pub struct ConfigInput {
    pub url: String,
    pub persist: String,
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
    pub api: ConfigApi,
    pub sources: Vec<ConfigSources>,
    pub working_dir: String,
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

    pub(crate) fn prepare(&mut self) {
        self.working_dir = get_working_path(&self.working_dir);
        self.api.prepare();
        self.prepare_api_web_root();
        for source in &mut self.sources {
            source.prepare();
            for target in &mut source.targets {
                target.prepare();
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
            api: self.api.clone(),
            sources: self.sources.clone(),
            working_dir: self.working_dir.clone(),
        }
    }
}

impl Clone for ConfigTarget {
    fn clone(&self) -> Self {
        ConfigTarget {
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
