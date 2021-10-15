use crate::utils::get_working_path;
use path_absolutize::*;
use crate::utils;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ItemField {
    Group,
    Name,
    Title,
}

impl std::fmt::Display for ItemField {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ItemField::Group => write!(f, "Group"),
            ItemField::Name => write!(f, "Name"),
            ItemField::Title => write!(f, "Title"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FilterMode {
    Discard,
    Include,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConfigFilter {
    pub field: ItemField,
    pub pattern: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub re: Option<regex::Regex>,
}

impl ConfigFilter {
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
pub struct ConfigFilters {
    pub mode: FilterMode,
    pub rules: Vec<ConfigFilter>,
}

impl ConfigFilters {
    pub(crate) fn is_include(&self) -> bool {
        matches!(self.mode, FilterMode::Include)
    }
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
pub struct ConfigTarget {
    pub filename: String,
    pub filter: ConfigFilters,
    pub rename: Vec<ConfigRename>,
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
    pub input: ConfigInput,
    pub targets: Vec<ConfigTarget>,
    pub working_dir: String,
}

impl Config {
    pub(crate) fn prepare(&mut self) {
        self.working_dir = get_working_path(&self.working_dir);
        self.api.prepare();
        self.prepare_api_web_root();
        for t in &mut self.targets {
            for f in &mut t.filter.rules {
                f.prepare();
            }
            for r in &mut t.rename {
                r.prepare();
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
        Config{
            api: self.api.clone(),
            input: self.input.clone(),
            targets: self.targets.clone(),
            working_dir: self.working_dir.clone(),
        }
    }
}

impl Clone for ConfigTarget {
    fn clone(&self) -> Self {
        ConfigTarget{
            filename: self.filename.clone(),
            filter: self.filter.clone(),
            rename: self.rename.clone()
        }
    }
}

impl Clone for ConfigFilters {
    fn clone(&self) -> Self {
        ConfigFilters{
            mode: self.mode.clone(),
            rules: self.rules.clone()
        }
    }
}

impl Clone for ConfigRename {
    fn clone(&self) -> Self {
        ConfigRename{
            field: self.field.clone(),
            pattern: self.pattern.clone(),
            new_name: self.new_name.clone(),
            re: None
        }
    }
}
