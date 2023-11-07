import React, {useCallback, useEffect, useRef, useState} from "react";
import './file-download.scss';
import {Subscription} from "rxjs";
import {DownloadErrorInfo, FileDownloadInfo} from "../../model/file-download";
import {useServices} from "../../provider/service-provider";
import {useSnackbar} from "notistack";
import {getIconByName} from "../../icons/icons";

const attachErrors = (errors: DownloadErrorInfo[], downloads: Record<string, FileDownloadInfo>): boolean => {
    errors.forEach(err => {
        const downl = downloads[err.uuid];
        if (downl) {
            downl.error = err.error;
        }
    });
    return !!errors.length;
}

interface FileDownloadProps {
}

export default function FileDownload(props: FileDownloadProps) {
    const services = useServices();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const [downloads, setDownloads] = useState<Record<string, FileDownloadInfo>>({})
    const downloading = useRef(false);

    const startPoll = useCallback(() => {
        if (!downloading.current) {
            downloading.current = true;
            let subs: Subscription = services.file().getDownloadInfo().subscribe({
                next: (info: FileDownloadInfo) => {
                    const errors: DownloadErrorInfo[] = info.errors;
                    info.errors = undefined;
                    info.ts = Date.now();
                    if (info.finished === true) {
                        downloading.current = false;
                        if (errors?.length) {
                            setDownloads((downloads: any) => {
                                if (attachErrors(errors, downloads)) {
                                   return {...downloads};
                                } else {
                                    return downloads;
                                }
                            });
                        }
                    } else {
                        setDownloads(downloads => {
                            downloads[info.uuid] = info;
                            attachErrors(errors, downloads);
                            return {...downloads};
                        });
                    }
                },
                error: (err) => {
                    enqueueSnackbar("Download file failed!", {variant: 'error'});
                    downloading.current = false;
                },
                complete: () => {
                    subs.unsubscribe();
                    downloading.current = false;
                }
            });
        }
    }, [enqueueSnackbar, services]);

    const handleDeleteClick = useCallback((event: any) => {
        const key = event.target.dataset.uuid;
        setDownloads(downloads => {
            delete downloads[key];
            return {...downloads};
        });
    }, []);

    useEffect(() => {
        const sub = services.file().subscribeDownloadNotification(() => startPoll());
        return () => sub.unsubscribe();
    }, [services, startPoll])

    const renderDownloads = useCallback((): React.ReactNode => {
        const info_list = Object.keys(downloads).map(key => downloads[key]);
        if (info_list.length) {
            info_list.sort((a, b) => b.ts - a.ts)
            return <div className={'download-info'}>
                <ul>
                    {info_list.map(download => {
                        return <li key={download.uuid}>
                            <span className={'download-info__remove_btn'} data-uuid={download.uuid}
                                  onClick={handleDeleteClick}>{getIconByName('Delete')}</span>
                            <span className={'download-info__content'}>{download.filename}:
                             <span
                                 className={'download-info__filesize'}>{download.filesize ? (download.filesize / 1_048_576).toFixed(2) : 0} MB</span>
                         </span>
                            {download.error && <span className={'download-info__error'}>{download.error}</span>}
                        </li>;
                    })
                    }
                </ul>
            </div>;
        }
        return <></>
    }, [downloads, handleDeleteClick]);

    return <div className={'file-download'}>
        {renderDownloads()}
    </div>;

}