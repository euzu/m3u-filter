export interface FileDownloadRequest {
    url: string;
    filename: string;
}

export interface FileDownloadResponse {
    download_id: string;
}

export interface FileDownloadInfo {
    download_id: string;
    filename?: string;
    finished?: boolean;
    filesize?: number;
}