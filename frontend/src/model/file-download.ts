export interface FileDownloadRequest {
    url: string;
    filename: string;
}

export interface FileDownloadResponse {
    success: boolean;
}

export interface DownloadErrorInfo {
    filename: string;
    error: string;
}


export interface FileDownloadInfo {
    filename?: string;
    finished?: boolean;
    filesize?: number;
    errors?: DownloadErrorInfo[];
}