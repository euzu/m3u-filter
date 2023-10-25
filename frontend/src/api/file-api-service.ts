import ApiService, {DefaultApiService} from "./api-service";
import {Observable, throwError} from "rxjs";
import {FileDownloadInfo, FileDownloadRequest, FileDownloadResponse} from "../model/file-download";
import {first} from "rxjs/operators";

//const FILE_API_PATH = 'file';
const FILE_DOWNLOAD_API_PATH = 'file/download';

export default interface FileApiService extends ApiService {
    download(req: FileDownloadRequest): Observable<FileDownloadResponse>;
    getDownloadInfo(download_id: string): Observable<FileDownloadInfo>;
}

export class DefaultFileApiService extends DefaultApiService implements FileApiService {
    download(req: FileDownloadRequest): Observable<FileDownloadResponse> {
        if (req.url) {
            return this.post<FileDownloadResponse>(FILE_DOWNLOAD_API_PATH, req);
        }
        return throwError(() => new Error('Invalid arguments'));
    }

    getDownloadInfo(download_id: string): Observable<FileDownloadInfo> {
        if (download_id) {
            return new Observable((observer) => {
                const fetch_info = () => {
                    this.get<FileDownloadInfo>(FILE_DOWNLOAD_API_PATH + '/' + download_id).pipe(first()).subscribe({
                        next: (info: FileDownloadInfo) => {
                            if (info.finished) {
                                observer.next(info);
                                observer.complete();
                            } else if (info.filesize != undefined) {
                                observer.next(info);
                                setTimeout(() =>  fetch_info(), 1000);
                            } else {
                                observer.error("unknown file download state");
                            }
                        },
                        error: (err) => observer.error(err)
                    });
                }
                fetch_info();
            });
        }
        return throwError(() => new Error('Invalid arguments'));
    }
}
