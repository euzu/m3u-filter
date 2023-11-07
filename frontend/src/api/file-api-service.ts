import ApiService, {DefaultApiService} from "./api-service";
import {Observable, throwError} from "rxjs";
import {FileDownloadInfo, FileDownloadRequest} from "../model/file-download";
import {first} from "rxjs/operators";

//const FILE_API_PATH = 'file';
const FILE_DOWNLOAD_API_PATH = 'file/download';
const FILE_DOWNLOAD_INFO_API_PATH =FILE_DOWNLOAD_API_PATH + '/info';

export default interface FileApiService extends ApiService {
    download(req: FileDownloadRequest): Observable<FileDownloadInfo>;
    getDownloadInfo(): Observable<FileDownloadInfo>;
}

export class DefaultFileApiService extends DefaultApiService implements FileApiService {
    download(req: FileDownloadRequest): Observable<FileDownloadInfo> {
        if (req.url) {
            return this.post<FileDownloadInfo>(FILE_DOWNLOAD_API_PATH, req);
        }
        return throwError(() => new Error('Invalid arguments'));
    }

    getDownloadInfo(): Observable<FileDownloadInfo> {
        return new Observable((observer) => {
            const fetch_info = () => {
                this.get<FileDownloadInfo>(FILE_DOWNLOAD_INFO_API_PATH).pipe(first()).subscribe({
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
                    error: (err) => observer.error(err),
                });
            }
            fetch_info();
        });
    }
}
