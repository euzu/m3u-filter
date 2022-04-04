use enum_iterator::IntoEnumIterator;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, IntoEnumIterator)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, IntoEnumIterator)]
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