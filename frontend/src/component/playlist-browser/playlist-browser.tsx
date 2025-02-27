import React, {useCallback, useMemo, useRef, useState} from 'react';
import './playlist-browser.scss';
import SourceSelector from "..//source-selector/source-selector";
import PlaylistViewer, {IPlaylistViewer, SearchRequest} from "../playlist-viewer/playlist-viewer";
import {useSnackbar} from 'notistack';
import {useServices} from "../../provider/service-provider";
import {first} from "rxjs/operators";
import Progress from '..//progress/progress';
import PlaylistFilter from "../playlist-filter/playlist-filter";
import {noop, Subject} from "rxjs";
import PlaylistVideo from "../playlist-video/playlist-video";
import ClipboardViewer from "../clipboard-viewer/clipboard-viewer";
import Sidebar from "../sidebar/sidebar";
import {PlaylistRequest} from "../../model/playlist-request";
import ServerConfig from "../../model/server-config";
import FileDownload from "../file-download/file-download";
import {FileDownloadInfo} from "../../model/file-download";
import useTranslator from "../../hook/use-translator";
import {EmptyPlaylistCategories, PlaylistCategories, PlaylistItem} from "../../model/playlist";

/* eslint-disable @typescript-eslint/no-empty-interface */
interface PlaylistBrowserProps {
    config: ServerConfig
}

export default function PlaylistBrowser(props: PlaylistBrowserProps) {
    const {config} = props;
    const searchChannel = useMemo<Subject<SearchRequest>>(() => new Subject<SearchRequest>(), []);
    const services = useServices();
    const [progress, setProgress] = useState<boolean>(false);
    const [playlist, setPlaylist] = useState<PlaylistCategories>(EmptyPlaylistCategories);
    const clipboardChannel = useMemo<Subject<string>>(() => new Subject<string>(), []);
    const viewerRef = useRef<IPlaylistViewer>(undefined);
    const translate = useTranslator();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const videoChannel = useMemo(() => new Subject<PlaylistItem>(), []);
    const handleSourceDownload = useCallback((req: PlaylistRequest) => {
        setProgress(true);
        services.playlist().getPlaylistCategories(req).pipe(first()).subscribe({
            next: (pl: PlaylistCategories) => {
                enqueueSnackbar(translate("MESSAGES.DOWNLOAD.PLAYLIST.SUCCESS"), {variant: 'success'})
                setPlaylist(pl);
            },
            error: (err) => {
                setProgress(false);
                enqueueSnackbar(translate("MESSAGES.DOWNLOAD.PLAYLIST.FAIL"), {variant: 'error'});
            },
            complete: () => setProgress(false),
        });
    }, [enqueueSnackbar, services, translate]);

    const handleFilter = useCallback((filter: string, regexp: boolean): void => {
        searchChannel.next({filter, regexp});
    }, [searchChannel]);

    const handleProgress = useCallback((value: boolean) => {
        setProgress(value);
    }, []);

    const handleOnPlay = useCallback((playlistItem: PlaylistItem): void => {
        videoChannel.next(playlistItem);
    }, [videoChannel]);

    const handleOnCopy = useCallback((playlistItem: PlaylistItem): void => {
        clipboardChannel.next(playlistItem.header.url);
    }, [clipboardChannel]);

    const handleOnDownload = useCallback((playlistItem: PlaylistItem): void => {
        let filename = playlistItem.header.title;
        const parts = playlistItem.header.url.split('.');
        let ext = undefined;
        if (parts.length > 1) {
            ext = parts[parts.length - 1];
        }

        if (config.video?.extensions?.includes(ext)) {
            services.file().download({
                url: playlistItem.header.url,
                filename: filename + '.' + ext
            }).pipe(first()).subscribe({
                next: (_: FileDownloadInfo) => {
                },
                error: _ => enqueueSnackbar(translate("MESSAGES.DOWNLOAD.FAIL"), {variant: 'error'}),
                complete: noop,
            });
        } else {
            enqueueSnackbar(translate("MESSAGES.INVALID_FILETYPE"), {variant: 'error'})
        }
    }, [config, enqueueSnackbar, services, translate]);

    const handleOnWebSearch = useCallback((playlistItem: PlaylistItem): void => {
        if (playlistItem) {
            let title = playlistItem.header.title;
            let pattern = config.video.download?.episode_pattern;
            if (pattern) {
                pattern = pattern.replace('?P<episode>', '?<episode>');
                const matches = title.match(pattern);
                if (matches?.groups?.episode) {
                    const idx = title.indexOf(matches?.groups?.episode);
                    title = title.substring(0, idx).trim();
                }
            }
            const dateSuffixMatch = title.match(/(.*?).\(\d+\)/);
            if (dateSuffixMatch?.length > 1) {
                title = dateSuffixMatch[1];
            }

            const url = config.video.web_search.replace("{}", title);
            window.open(url, "imdb");
        }
    }, [config]);


    return (<>
        <Sidebar>
            <ClipboardViewer channel={clipboardChannel}></ClipboardViewer>
        </Sidebar>
        <div className={'playlist-browser__content'}>
            <div className={'playlist-browser__toolbar'}>
                <PlaylistFilter onFilter={handleFilter}/>
                <SourceSelector onDownload={handleSourceDownload} serverConfig={config}/>
            </div>
            <PlaylistViewer ref={viewerRef} playlist={playlist} searchChannel={searchChannel}
                            onProgress={handleProgress} onCopy={handleOnCopy} onPlay={handleOnPlay}
                            onDownload={handleOnDownload}
                            onWebSearch={handleOnWebSearch}
                            serverConfig={config}/>
            <PlaylistVideo channel={videoChannel}/>
            <FileDownload></FileDownload>
            <Progress visible={progress}/>
        </div>
    </>);
}
