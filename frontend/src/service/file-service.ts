import {Observable, Subject, Subscription, tap} from "rxjs";
import {DownloadInfo, FileDownloadInfo, FileDownloadRequest} from "../model/file-download";
import FileApiService, { DefaultFileApiService } from "../api/file-api-service";

export default class FileService {

    private downloadNotification = new Subject<FileDownloadInfo>();
    constructor(private fileApiService: FileApiService = new DefaultFileApiService()) {
    }

    subscribeDownloadNotification<T>(observer: (value: T) => void): Subscription {
        return this.downloadNotification.subscribe(observer as any);
    }

    private notifyDownload(info: FileDownloadInfo) {
        this.downloadNotification.next(info);
    }

    download(req: FileDownloadRequest): Observable<FileDownloadInfo> {
        return this.fileApiService.download(req).pipe(tap((result) => this.notifyDownload(result) ));
    }

    getDownloadInfo(): Observable<DownloadInfo> {
        return this.fileApiService.getDownloadInfo();
    }
}

