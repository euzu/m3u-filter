import {PlaylistItem, PlaylistGroup} from "../model/playlist";
import FileSaver from "file-saver";
import {Observable, Subject, Subscription, tap} from "rxjs";
import FileApiService, {DefaultFileApiService} from "../api/file-api-service";
import {DownloadInfo, FileDownloadInfo, FileDownloadRequest} from "../model/file-download";

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
    //
    // save(playlist: PlaylistGroup[]) {
    //     const lines = ['#EXTM3U'];
    //     playlist.forEach(group => {
    //         group.channels.forEach((entry: PlaylistItem) => {
    //             lines.push(entry.header.source);
    //             lines.push(entry.header.url);
    //         })
    //     });
    //     const blob = new Blob([lines.join('\n')], { type: "text/plain;charset=utf-8" });
    //     FileSaver.saveAs(blob, "playlist.m3u");
    // }

    download(req: FileDownloadRequest): Observable<FileDownloadInfo> {
        return this.fileApiService.download(req).pipe(tap((result) => this.notifyDownload(result) ));
    }

    getDownloadInfo(): Observable<DownloadInfo> {
        return this.fileApiService.getDownloadInfo();
    }
}

