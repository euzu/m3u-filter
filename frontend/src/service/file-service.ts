import {PlaylistItem, PlaylistGroup} from "../model/playlist";
import FileSaver from "file-saver";
import {Observer, Observable, Subject, Subscription, tap} from "rxjs";
import FileApiService, {DefaultFileApiService} from "../api/file-api-service";
import {FileDownloadInfo, FileDownloadRequest, FileDownloadResponse} from "../model/file-download";

export default class FileService {

    private downloadNotification = new Subject<boolean>();
    constructor(private fileApiService: FileApiService = new DefaultFileApiService()) {
    }

    subscribeDownloadNotification<T>(observer: (value: T) => void): Subscription {
        return this.downloadNotification.subscribe(observer as any);
    }

    private notifyDownload() {
        this.downloadNotification.next(true);
    }

    save(playlist: PlaylistGroup[]) {
        const lines = ['#EXTM3U'];
        playlist.forEach(group => {
            group.channels.forEach((entry: PlaylistItem) => {
                lines.push(entry.header.source);
                lines.push(entry.header.url);
            })
        });
        const blob = new Blob([lines.join('\n')], { type: "text/plain;charset=utf-8" });
        FileSaver.saveAs(blob, "playlist.m3u");
    }

    download(req: FileDownloadRequest): Observable<FileDownloadResponse> {
        return this.fileApiService.download(req).pipe(tap((result) => result?.success && this.notifyDownload() ));
    }

    getDownloadInfo(): Observable<FileDownloadInfo> {
        return this.fileApiService.getDownloadInfo();
    }
}

