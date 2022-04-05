use path_absolutize::*;

use crate::filter::{Filter, get_filter, ValueProvider};
use crate::model::{ItemField, SortOrder, TargetType};
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
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigTarget {
    pub filename: String,
    pub options: Option<ConfigOptions>,
    pub sort: Option<ConfigSort>,
    pub filter: String,
    #[serde(alias = "type")]
    pub output: Option<TargetType>,
    pub rename: Vec<ConfigRename>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _filter: Option<Filter>,
}

impl ConfigTarget {
    pub(crate) fn prepare(&mut self) -> () {
        self._filter = Some(get_filter(&self.filter));
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigInput {
    pub url: String,
    pub persist: String,
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
    pub(crate) fn prepare(&mut self) {
        self.working_dir = get_working_path(&self.working_dir);
        self.api.prepare();
        self.prepare_api_web_root();
        for source in &mut self.sources {
            for target in &mut source.targets {
                target.prepare();
                for r in &mut target.rename {
                    r.prepare();
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
            _filter: self._filter.clone(),
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
