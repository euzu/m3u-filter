import React, {useCallback, useState, useRef, useEffect} from 'react';
import './playlist-tree.scss';
import {PlaylistGroup, PlaylistItem} from "../../model/playlist";
import copyToClipboard from "../../utils/clipboard";
import {first} from "rxjs/operators";
import {noop, Subscription} from "rxjs";
import {useSnackbar} from "notistack";
import {getIconByName} from "../../icons/icons";
import {useServices} from "../../provider/service-provider";
import ServerConfig from "../../model/server-config";
import {FileDownloadInfo, FileDownloadResponse} from "../../model/file-download";

const VALID_VIDEO_FILES = ['mkv', 'mp4', 'avi'];

type DownloadInfo = {filename: string, filesize: number};

export type PlaylistTreeState = { [key: number]: boolean };

interface PlaylistTreeProps {
    serverConfig: ServerConfig;
    data: PlaylistGroup[];
    state: PlaylistTreeState;
    onCopy: (playlistItem: PlaylistItem) => void;
    onPlay?: (playlistItem: PlaylistItem) => void;
}

export default function PlaylistTree(props: PlaylistTreeProps) {
    const {serverConfig, state, data, onCopy, onPlay} = props;

    const [, setForceUpdate] = useState(null);
    const expanded = useRef<PlaylistTreeState>({});
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const services = useServices();
    const [videoExtensions, setVideoExtensions] = useState<string[]>([]);
    const [downloads, setDownloads] = useState<Record<string, DownloadInfo>>({})

    useEffect(() => {
        if (serverConfig) {
            setVideoExtensions(serverConfig.video.extensions);
        }
        return noop;
    }, [serverConfig]);


    const setDownloadsInfo = useCallback((info: FileDownloadInfo) => {
        if (info.finished == undefined && info.filesize == undefined) {
            setDownloads((downloads) => {
                downloads[info.download_id] = {filename: info.filename, filesize: 0};
                return {...downloads};
            });
        } else {
            if (info.filesize != undefined) {
                setDownloads((downloads) => {
                    downloads[info.download_id].filesize = info.filesize;
                    return {...downloads};
                });
            } else {
                setDownloads((downloads) => {
                    delete downloads[info.download_id];
                    return {...downloads};
                });
            }
        }
    }, []);

    const getPlaylistItemById = useCallback((itemId: string): PlaylistItem => {
        const id = parseInt(itemId);
        if (data && !isNaN(id)) {
            for (let i = 0, len = data.length; i < len; i++) {
                const group = data[i];
                for (let j = 0, clen = group.channels.length; j < clen; j++) {
                    const plitem = group.channels[j];
                    if (plitem.id == id) {
                        return plitem;
                    }
                }
            }
        }
        return undefined;
    }, [data]);

    const handleChange = useCallback((event: any) => {
        const key = event.target.dataset.group;
        state[key] = !state[key];
        setForceUpdate({});
    }, [state]);

    const handleExpand = useCallback((event: any) => {
        const key = event.target.dataset.group;
        expanded.current[key] = !expanded.current[key];
        setForceUpdate({});
    }, []);

    const handleClipboardUrl = useCallback((e: any) => {
        const item = getPlaylistItemById(e.target.dataset.item);
        if (item) {
            onCopy(item);
            copyToClipboard(item.header.url).pipe(first()).subscribe({
                next: value => enqueueSnackbar(value ? "URL copied to clipboard" : "Copy to clipboard failed!", {variant: value ? 'success' : 'error'}),
                error: err => enqueueSnackbar("Copy to clipboard failed!", {variant: 'error'}),
                complete: noop,
            });
        }
    }, [enqueueSnackbar, getPlaylistItemById, onCopy]);

    const startPollingDownload = useCallback((downloadId: string) => {
        let subs: Subscription = services.file().getDownloadInfo(downloadId).subscribe({
            next: (info: FileDownloadInfo) => setDownloadsInfo(info),
            error: (err) => enqueueSnackbar("Download file failed!", {variant: 'error'}),
            complete: () => subs.unsubscribe()
        });
    },  [setDownloadsInfo, enqueueSnackbar, services]);

    const handleDownloadUrl = useCallback((e: any) => {
        if (! serverConfig.video.download?.directory) {
            enqueueSnackbar("Please updated the server configuration and add video.download directory and headers!", {variant: 'error'})
        } else {
            const item = getPlaylistItemById(e.target.dataset.item);
            if (item) {
                let filename = item.header.title;
                const parts = item.header.url.split('.');
                let ext = undefined;
                if (parts.length > 1) {
                    ext = parts[parts.length - 1];
                }

                if (VALID_VIDEO_FILES.includes(ext)) {
                    filename = filename + '.' + ext;
                    services.file().download({url: item.header.url, filename}).pipe(first()).subscribe({
                        next: (download: FileDownloadResponse) => {
                            setDownloadsInfo({download_id: download.download_id, filename: filename});
                            startPollingDownload(download.download_id)
                        },
                        error: err => enqueueSnackbar("Download failed!", {variant: 'error'}),
                        complete: noop,
                    });
                } else {
                    enqueueSnackbar("Invalid filetype!", {variant: 'error'})
                }
            }
        }
    }, [serverConfig, enqueueSnackbar, getPlaylistItemById, services, startPollingDownload, setDownloadsInfo]);

    const handlePlayUrl = useCallback((e: any) => {
        if (onPlay) {
            const item = getPlaylistItemById(e.target.dataset.item);
            if (item) {
                onPlay(item);
            }
        }
    }, [onPlay, getPlaylistItemById]);

    const isVideoFile = useCallback((entry: PlaylistItem): boolean => {
            if (videoExtensions && entry.header.url) {
                for (const ext of videoExtensions) {
                    if (entry.header.url.endsWith(ext)) {
                        return true;
                    }
                }
            }
            return false;
    }, [videoExtensions]);

    const renderEntry = useCallback((entry: PlaylistItem, index: number): React.ReactNode => {
        return <div key={entry.id} className={'tree-channel'}>
            <div className={'tree-channel-tools'}>
                <div className={'tool-button'} data-item={entry.id} onClick={handleClipboardUrl}>
                    {getIconByName('LinkRounded')}
                </div>
                <div style={{display: 'none'}} className={'tool-button'} data-item={entry.id} onClick={handlePlayUrl}>
                    {getIconByName('PlayArrow')}
                </div>
                {isVideoFile(entry) &&
                    <div className={'tool-button'} data-item={entry.id} onClick={handleDownloadUrl}>
                        {getIconByName('Download')}
                    </div>
                }
            </div>
            <div className={'tree-channel-content'}>
                <div className={'tree-channel-nr'}>{index + 1}</div>
                {entry.header.name}</div>
        </div>
    }, [handleClipboardUrl, handlePlayUrl, handleDownloadUrl, isVideoFile]);

    const renderGroup = useCallback((group: PlaylistGroup): React.ReactNode => {
        return <div className={'tree-group'} key={group.id}>
            <div className={'tree-group-header'}>
                <div className={'tree-expander'} data-group={group.id}
                     onClick={handleExpand}>{getIconByName(expanded.current[group.id] ?
                    'ExpandMore' : 'ChevronRight')}</div>
                <div className={'tree-group-header-content'}>
                    <input type={"checkbox"} onChange={handleChange} data-group={group.id}/>
                    {group.title}
                    <div className={'tree-group-count'}>({group.channels.length})</div>
                </div>
            </div>
            {expanded.current[group.id] && (
                <div className={'tree-group-childs'}>
                    {group.channels.map(renderEntry)}
                </div>)}
        </div>;
    }, [handleChange, handleExpand, renderEntry]);

    const renderPlaylist = useCallback((): React.ReactNode => {
        if (!data) {
            return <React.Fragment/>;
        }
        return <React.Fragment>
            {data.map(renderGroup)}
        </React.Fragment>;
    }, [data, renderGroup]);

    const renderDownloads = useCallback((): React.ReactNode => {
        const keys = Object.keys(downloads)
        if (keys.length) {
            let elements = keys.map(key => {
                const info: DownloadInfo = downloads[key];
                return <li key={key}>{info.filename}: {info.filesize ?  (info.filesize / 1_048_576).toFixed(2) : 0} MB</li>;
            })
            return <div className={'download-info'}><ul>{elements}</ul></div>;
        }
        return <></>;
    }, [downloads]);

    return <div className={'playlist-tree'}>{renderPlaylist()}{renderDownloads()}</div>;
} 