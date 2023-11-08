import ApiService, {DefaultApiService} from "./api-service";
import {Observable, throwError} from "rxjs";
import {DownloadInfo, FileDownloadInfo, FileDownloadRequest} from "../model/file-download";
import {first} from "rxjs/operators";

//const FILE_API_PATH = 'file';
const FILE_DOWNLOAD_API_PATH = 'file/download';
const FILE_DOWNLOAD_INFO_API_PATH = FILE_DOWNLOAD_API_PATH + '/info';

export default interface FileApiService extends ApiService {
    download(req: FileDownloadRequest): Observable<FileDownloadInfo>;

    getDownloadInfo(): Observable<DownloadInfo>;
}

export class DefaultFileApiService extends DefaultApiService implements FileApiService {
    download(req: FileDownloadRequest): Observable<FileDownloadInfo> {
        if (req.url) {
            return this.post<FileDownloadInfo>(FILE_DOWNLOAD_API_PATH, req);
        }
        return throwError(() => new Error('Invalid arguments'));
    }

    getDownloadInfo(): Observable<DownloadInfo> {
        return new Observable((observer) => {
            const fetch_info = () => {
                this.get<DownloadInfo>(FILE_DOWNLOAD_INFO_API_PATH).pipe(first()).subscribe({
                    next: (info: DownloadInfo) => {
                        if (info.completed) {
                            observer.next(info);
                            observer.complete();
                        } else {
                            observer.next(info);
                            setTimeout(() => fetch_info(), 1000);
                        }
                    },
                    error: (err) => observer.error(err),
                });
            }
            fetch_info();
        });
    }
}
