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

export interface PlaylistItem {
    id: number;
    header: PlaylistItemHeader;
}

export interface PlaylistGroup {
    id: number;
    title: string;
    channels: PlaylistItem[];
}