export interface PlaylistItemHeader {
    id: number;
    name: string;
    logo: string;
    logo_small: string;
    group: string;
    title: string;
    parent_code: string;
    audio_track: string;
    time_shift: string;
    rec: string;
    source: string;
    url: string;
}

export enum XtreamCluster {
    Live = 'Live',
    Video = 'Video',
    Series = 'Series',
}

export enum PlaylistItemType {
    Live = 'Live',
    Video = 'Video',
    Series = 'Series', //  xtream series description
    SeriesInfo = 'SeriesInfo', //  xtream series info fetched for series description
    Catchup = 'Catchup',
    LiveUnknown = 'LiveUnknown', // No Provider id
    LiveHls = 'LiveHls', // m3u8 entry
}

export interface PlaylistItem {
    id: number;
    category_id: number,
    provider_id: number,
    virtual_id: number,
    url: string,
    name: string,
    title: string,
    channel_no: number,
    epg_channel_id: string,
    group: string,
    input_name: string,
    item_type: PlaylistItemType,
    logo: string,
    logo_small: string,
    xtream_cluster: XtreamCluster,
    additional_properties: string,
}

export interface PlaylistGroup {
    id: number,
    name: string,
    channels?: PlaylistItem[];
}

export interface PlaylistCategories {
    live: PlaylistGroup[],
    vod: PlaylistGroup[],
    series: PlaylistGroup[],
}

export enum PlaylistCategory {
    LIVE = 'live',
    VOD = 'vod',
    SERIES = 'series'
}

export const EmptyPlaylistCategories: PlaylistCategories = {live: [], vod: [], series: []}


export interface UserPlaylistTargetCategories {
    live: string[],
    vod: string[],
    series: string[],
}

export interface UserPlaylistCategories {
    xtream: UserPlaylistTargetCategories,
    m3u: UserPlaylistTargetCategories,
}