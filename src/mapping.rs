fn default_as_false() -> bool { false }
fn default_as_empty_str() -> String { String::from("") }

#[derive(Debug)]
pub struct MapperRe {
    pub re: regex::Regex,
    pub captures: Vec<String>,
}

impl Clone for MapperRe {
    fn clone(&self) -> Self {
        MapperRe {
            re: self.re.clone(),
            captures: self.captures.clone()
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mapper {
    pub tvg_name: String,
    pub tvg_names: Vec<String>,
    pub tvg_id: String,
    pub tvg_chno: String,
    pub tvg_logo: String,
    pub group_title: Vec<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub _re: Option<Vec<MapperRe>>,
}

impl Mapper {
    pub(crate) fn prepare(&mut self, templates: Option<&Vec<MapperTemplate>>) -> () {
        let mut rev = Vec::new();
        for rs in &self.tvg_names {
            let mut regstr = String::from(rs);
            let templ : &Vec<MapperTemplate> = templates.unwrap();
            for t in templ {
                regstr = regstr.replace(format!("!{}!", &t.name).as_str(), &t.value);
            }

            let re = regex::Regex::new(regstr.as_str());
            if re.is_err() {
                println!("cant parse regex: {}", &regstr);
                std::process::exit(1);
            } else {
                let regexp = re.unwrap();
                let captures = regexp.capture_names().map(|x| String::from(x.unwrap_or(""))).filter(|x| x.len() > 0).collect::<Vec<String>>();
                rev.push(MapperRe {
                    re: regexp,
                    captures
                })
            }
        }
        self._re = Some(rev);
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
            _re: self._re.clone(),
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MapperTemplate {
    pub name: String,
    pub value: String,
}

impl Clone for MapperTemplate {
    fn clone(&self) -> Self {
        MapperTemplate {
            name: self.name.clone(),
            value: self.value.clone()
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Mapping {
    pub id: String,
    pub tag: Option<MappingTag>,
    #[serde(default = "default_as_false")]
    pub match_as_ascii: bool,
    pub templates: Option<Vec<MapperTemplate>>,
    pub mapper: Vec<Mapper>,
}

impl Clone for Mapping {
    fn clone(&self) -> Self {
        Mapping {
            id: self.id.clone(),
            tag: self.tag.clone(),
            match_as_ascii: self.match_as_ascii,
            templates: self.templates.clone(),
            mapper: self.mapper.clone(),
        }
    }
}

impl Mapping {
    pub(crate) fn prepare(&mut self) -> () {
        for mapper in &mut self.mapper {
            match &self.templates {
              Some(templ) => mapper.prepare(Some(&templ)),
                _ => mapper.prepare(None)
            }
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
