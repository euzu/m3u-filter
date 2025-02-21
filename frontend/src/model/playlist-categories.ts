export interface PlaylistCategory {
    id: number,
    name: string,
}

export interface PlaylistCategories {
    live: PlaylistCategory[],
    vod: PlaylistCategory[],
    series: PlaylistCategory[],
}