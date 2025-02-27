export enum PlaylistRequestType {
    INPUT = 1,
    TARGET = 2,
    XTREAM= 3,
    M3U= 4
}

export interface PlaylistRequest {
    rtype: PlaylistRequestType;
    sourceName?: string;
    username?: string;
    password?: string;
    url?: string;
    sourceId?: number,
}