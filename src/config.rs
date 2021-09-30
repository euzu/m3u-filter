#[derive(Debug, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum FilterMode {
    Discard,
    Include,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ConfigInput {
    pub url: String,
    pub persist: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Config {
    pub input: ConfigInput,
    pub targets: Vec<ConfigTarget>,
}

impl Config {
    pub(crate) fn prepare(&mut self) {
        for t in &mut self.targets {
            for f in &mut t.filter.rules {
                f.prepare();
            }
            for r in &mut t.rename {
                r.prepare();
            }
        }
    }
}
