export interface FileDownloadRequest {
    url: string;
    filename: string;
}

export interface DownloadErrorInfo {
    uuid: string;
    filename: string;
    error: string;
}


export interface FileDownloadInfo {
    uuid: string;
    ts?: number;
    filename?: string;
    finished?: boolean;
    filesize?: number;
    error?: string;
    errors?: DownloadErrorInfo[];
}