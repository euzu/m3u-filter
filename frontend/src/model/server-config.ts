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
    download: {
        headers: Record<string, string>,
        directory: string;
    }
}

export default interface ServerConfig {
    sources: SourceConfig[];
    video: VideoConfig;
}