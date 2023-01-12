#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mapper {
    pub tvg_name: String,
    pub tvg_names: Vec<String>,
    pub tvg_id: String,
    pub tvg_chno: String,
    pub tvg_logo: String,
    pub group_title: Vec<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub re: Option<Vec<regex::Regex>>,
}

impl Mapper {
    pub(crate) fn prepare(&mut self) -> () {
        let mut rev = Vec::new();
        for regstr in &self.tvg_names {
            let re = regex::Regex::new(regstr);
            if re.is_err() {
                println!("cant parse regex: {}", &regstr);
                std::process::exit(1);
            } else {
                rev.push(re.unwrap())
            }
        }
        self.re = Some(rev);
    }
}

impl Clone for Mapper {
    fn clone(&self) -> Self {
        Mapper {
            tvg_name: self.tvg_name.clone(),
            tvg_names: self.tvg_names.clone(),
            tvg_id: self.tvg_id.clone(),
            tvg_chno: self.tvg_chno.clone(),
            tvg_logo: self.tvg_logo.clone(),
            group_title: self.group_title.clone(),
            re: None,
        }
    }
}


#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mapping {
    pub id: String,
    pub tag: String,
    pub mapper: Vec<Mapper>,
}

impl Clone for Mapping {
    fn clone(&self) -> Self {
        Mapping {
            id: self.id.clone(),
            tag: self.tag.clone(),
            mapper: self.mapper.clone(),
        }
    }
}

impl Mapping {
    pub(crate) fn prepare(&mut self) -> () {
        for mapper in &mut self.mapper {
            mapper.prepare()
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mappings {
    pub mappings: Vec<Mapping>
}

impl Mappings {
    pub(crate) fn prepare(&mut self) {
        for mapping in &mut self.mappings {
            mapping.prepare();
        }
    }

    pub(crate) fn get_mapping(&self, mapping_id: &String) -> Option<Mapping> {
        for mapping in &self.mappings {
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
