use enum_iterator::Sequence;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Sequence)]
pub enum TargetType {
    M3u,
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
    FRM,
    FMR,
    RFM,
    RMF,
    MFR,
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
    Group,
    Name,
    Title,
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
    Discard,
    Include,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum SortOrder {
    Asc,
    Desc,
}