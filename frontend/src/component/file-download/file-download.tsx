import React, {useCallback, useEffect, useRef, useState} from "react";
import './file-download.scss';
import {Subscription} from "rxjs";
import {DownloadInfo, FileDownloadInfo} from "../../model/file-download";
import {useServices} from "../../provider/service-provider";
import {useSnackbar} from "notistack";
import {getIconByName} from "../../icons/icons";

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
                next: (info: DownloadInfo) => {
                    if (info.completed === true) {
                        downloading.current = false;
                    }
                    const new_downloads = info.downloads || [];
                    if (info.active) {
                        new_downloads.push(info.active);
                    }
                    if (new_downloads.length) {
                        setDownloads(downloads => {
                            new_downloads.forEach(d => downloads[d.uuid] = d);
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

    const handleClearAll = useCallback(() => {
        setDownloads({});
    }, []);

    useEffect(() => {
        const sub = services.file().subscribeDownloadNotification((info: FileDownloadInfo) => {
            setDownloads((downloads: any) => {
                if (!downloads[info.uuid]) {
                    info.ts_created = Date.now();
                    info.ts_modified = info.ts_created;
                    downloads[info.uuid] = info;
                    return {...downloads};
                }
                return downloads;
            });
            startPoll();
        });
        return () => sub.unsubscribe();
    }, [services, startPoll])

    const renderDownloads = useCallback((): React.ReactNode => {
        const info_list = Object.keys(downloads).map(key => downloads[key]);
        if (info_list.length) {
            info_list.sort((a, b) => {
                const ats = a.finished ? a.ts_created : a.ts_modified;
                const bts = b.finished ? b.ts_created : b.ts_modified;
                return bts - ats;
            })
            return <div className={'file-download'}>
                <div className={'download-info'}>
                    <div className={'download-info__toolbar'}>
                        <button title={'Clear All'} onClick={handleClearAll}>{getIconByName('DeleteSweep')}</button>
                    </div>
                    <div className={'download-info__content'}>
                        <ul>
                            {info_list.map(download => {
                                return <li key={download.uuid}>
                                <span className={'download-info__remove_btn'} data-uuid={download.uuid}
                                      onClick={handleDeleteClick}>{getIconByName('Delete')}
                                </span>
                                    <span
                                        className={'download-info__status'}>{getIconByName(download.error ? 'Error' : (download.finished ? 'CheckMark' : 'Hourglass'))}</span>
                                    <span className={'download-info__content'}>{download.filename}:
                                    <span
                                        className={'download-info__filesize'}>{download.filesize ? (download.filesize / 1_048_576).toFixed(2) : 0} MB
                                    </span>
                                 </span>
                                    {download.error && <span className={'download-info__error'}>{download.error}</span>}
                                </li>;
                            })
                            }
                        </ul>
                    </div>
                </div>
            </div>;
        }
        return <></>
    }, [downloads, handleDeleteClick, handleClearAll]);

    return renderDownloads();
}