export enum InputType {
    m3u = "m3u",
    xtream = "xtream"
}

export enum SortOrder {
    asc = "asc",
    desc = "desc"
}

export enum TargetType {
    m3u = "m3u",
    xtream = "xtream",
    strm = "strm"
}

export enum ProcessingOrder {
    frm = "frm",
    fmr = "fmr",
    rfm = "rfm",
    rmf = "rmf",
    mfr = "mfr",
    mrf = "mrf",
}

export interface InputConfig {
    id: number,
    input_type: InputType,
    url: string,
    username: string,
    password: string,
    persist: string,
    name: string,
    enabled: boolean
    options: {
        xtream_skip_live: boolean,
        xtream_skip_vod: boolean,
        xtream_skip_series: boolean,
    },
}

export interface TargetConfig {
    enabled: boolean,
    name: string,
    options: {
        ignore_logo: boolean,
        underscore_whitespace: boolean,
        cleanup: boolean,
        kodi_style: boolean,
        xtream_skip_live_direct_source: boolean,
        xtream_skip_video_direct_source: boolean,
        xtream_skip_series_direct_source: boolean,
        xtream_resolve_series: boolean,
        xtream_resolve_series_delay: number,
        xtream_resolve_video: boolean,
        xtream_resolve_video_delay: number,
        m3u_include_type_in_url: boolean,
        m3u_mask_redirect_url: boolean,
        share_live_streams: boolean,
    },
    sort: {
        match_as_ascii: boolean,
        groups: {
            order: SortOrder
        },
        channels:
            {
                field: string,
                group_pattern: string,
                order: SortOrder
            }[]
    },
    filter: string,
    output: [
        {
            target: TargetType,
            filename: string
        }
    ],
    rename: [
        {
            field: string,
            pattern: string,
            new_name: string
        }
    ],
    mapping: string[],
    processing_order: ProcessingOrder,
    watch: string[]
}

export interface SourceConfig {
    inputs: InputConfig[];
    targets: TargetConfig[];
}

export interface VideoDownloadConfig {
    headers: Record<string, string>,
    directory: string;
    organize_into_directories: boolean;
    episode_pattern: string;
}

export interface VideoConfig {
    extensions: string[];
    download?: VideoDownloadConfig,
    web_search?: string;
}

export interface TelegramConfig {
    bot_token: string;
    chat_ids: string[];
}

export interface MessagingConfig {
    notify_on?: string[];
    telegram?: TelegramConfig;
}

export interface Credentials {
    username: string;
    password: string;
    token: string;
    proxy: 'redirect' | 'reverse';
}

export interface TargetUser {
    target: string;
    credentials: Credentials[];
}

export interface ApiProxyServerInfo {
    name: string;
    protocol: string;
    host: string;
    http_port: string;
    https_port: string;
    rtmp_port: string;
    timezone: string;
    message: string;
}

export interface ApiProxyConfig {
    server: ApiProxyServerInfo[];
    user: TargetUser[];
}

export interface ServerApiConfig {
    host: string;
    port: number;
    web_root: string
}

export interface ServerMainConfig {
    api: ServerApiConfig;
    threads: number;
    working_dir: string;
    backup_dir: string;
    schedule: string;
    messaging?: MessagingConfig;
    video?: VideoConfig;
}

export default interface ServerConfig extends ServerMainConfig {
    sources: SourceConfig[];
    api_proxy?: ApiProxyConfig;
}