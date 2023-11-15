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
}

export interface TargetConfig {
    enabled: boolean,
    name: string,
    options: {
        ignore_logo: boolean,
        underscore_whitespace: boolean,
        cleanup: boolean,
        kodi_style: boolean
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

export interface VideoConfig {
    extensions: string[];
    download?: {
        headers: Record<string, string>,
        directory: string;
        organize_into_directories: boolean;
        episode_pattern: string;
    }
    web_search?: string;
}

export interface MessaginConfig {
    notify_on?: string [];
    telegram?: {
        bot_token: string;
        chat_ids: string[];
    }
}

export interface Credentials {
    username: string;
    password: string;
    token: string;
}

export interface TargetUser {
    target: string;
    credentials: Credentials[];
}

export interface ServerInfo {
    protocol: string;
    ip: string;
    http_port: string;
    https_port: string;
    rtmp_port: string;
    timezone: string;
    message: string;

}

export interface ApiProxyConfig {
    server: ServerInfo;
    user: TargetUser[];
}

export default interface ServerConfig {
    api: {host: string, port: number, web_root: string};
    threads: number;
    working_dir: string;
    schedule: string;
    messaging?: MessaginConfig;
    video?: VideoConfig;
    sources: SourceConfig[];
    api_proxy?: ApiProxyConfig;
}