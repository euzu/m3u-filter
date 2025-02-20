export interface PlaylistCategory {
    category_id: number,
    category_name: string,
    parent_id: number,
}

export interface PlaylistCategories {
    live: PlaylistCategory[],
    vod: PlaylistCategory[],
    series: PlaylistCategory[],
}