#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct PlaylistCategories {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vod: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<Vec<String>>,
}


#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct PlaylistCategoryDto {
    pub id: String,
    pub name: String,
}
#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct PlaylistCategoriesDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<Vec<PlaylistCategoryDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vod: Option<Vec<PlaylistCategoryDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<Vec<PlaylistCategoryDto>>,
}


#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct TargetBouquetDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub live: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vod: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<Vec<String>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct PlaylistBouquetDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xtream: Option<TargetBouquetDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub m3u: Option<TargetBouquetDto>,
}