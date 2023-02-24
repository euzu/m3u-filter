use enum_iterator::Sequence;

pub const MAPPER_ATTRIBUTE_FIELDS: &[&str] = &["name", "title", "group", "id", "chno"];
pub const MAPPER_PREFIX_SUFFIX_FIELDS: &[&str] = &["name", "title", "group"];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence)]
pub enum TargetType {
    #[serde(rename = "m3u")]
    M3u,
    #[serde(rename = "strm")]
    Strm,
}

impl std::fmt::Display for TargetType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            TargetType::M3u => write!(f, "M3u"),
            TargetType::Strm => write!(f, "Strm"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence)]
pub enum ProcessingOrder {
    #[serde(rename = "frm")]
    FRM,
    #[serde(rename = "fmr")]
    FMR,
    #[serde(rename = "rfm")]
    RFM,
    #[serde(rename = "rmf")]
    RMF,
    #[serde(rename = "mfr")]
    MFR,
    #[serde(rename = "mrf")]
    MRF
}

impl std::fmt::Display for ProcessingOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ProcessingOrder::FRM => write!(f, "filter, rename, map"),
            ProcessingOrder::FMR => write!(f, "filter, map, rename"),
            ProcessingOrder::RFM => write!(f, "rename, filter, map"),
            ProcessingOrder::RMF => write!(f, "rename, map, filter"),
            ProcessingOrder::MFR => write!(f, "map, filter, rename"),
            ProcessingOrder::MRF => write!(f, "map, rename, filter"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence)]
pub enum ItemField {
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "name")]
    Name,
    #[serde(rename = "title")]
    Title,
    #[serde(rename = "url")]
    Url,
}

impl std::fmt::Display for ItemField {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            ItemField::Group => write!(f, "Group"),
            ItemField::Name => write!(f, "Name"),
            ItemField::Title => write!(f, "Title"),
            ItemField::Url => write!(f, "Url"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum FilterMode {
    #[serde(rename = "discard")]
    Discard,
    #[serde(rename = "include")]
    Include,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SortOrder {
    #[serde(rename = "asc")]
    Asc,
    #[serde(rename = "desc")]
    Desc,
}