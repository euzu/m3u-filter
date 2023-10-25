import React, {useRef, useState, useCallback, useMemo, useEffect} from 'react';
import './app.scss';
import SourceSelector from "../component/source-selector/source-selector";
import PlaylistViewer, {IPlaylistViewer} from "../component/playlist-viewer/playlist-viewer";
import {useSnackbar} from 'notistack';
import Toolbar from "../component/toolbar/toolbar";
import {PlaylistGroup, PlaylistItem} from "../model/playlist";
import {useServices} from "../provider/service-provider";
import {first} from "rxjs/operators";
import Progress from '../component/progress/progress';
import PlaylistFilter from "../component/playlist-filter/playlist-filter";
import {noop, Subject} from "rxjs";
import PlaylistVideo from "../component/playlist-video/playlist-video";
import ClipboardViewer from "../component/clipboard-viewer/clipboard-viewer";
import Sidebar from "../component/sidebar/sidebar";
import {PlaylistRequest} from "../model/playlist-request";
import ServerConfig from "../model/server-config";

/* eslint-disable @typescript-eslint/no-empty-interface */
interface AppProps {

}

export default function App(props: AppProps) {
    const searchChannel = useMemo<Subject<string>>(() => new Subject<string>(), []);
    const [progress, setProgress] = useState<boolean>(false);
    const [playlist, setPlaylist] = useState<PlaylistGroup[]>([]);
    const [serverConfig, setServerConfig] = useState<ServerConfig>(undefined);
    const clipboardChannel = useMemo<Subject<string>>(() => new Subject<string>(), []);
    const viewerRef = useRef<IPlaylistViewer>();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const services = useServices();
    const videoChannel = useMemo(() => new Subject<PlaylistItem>(), []);
    const handleDownload = useCallback((req: PlaylistRequest) => {
        setProgress(true);
        services.playlist().getPlaylist(req).pipe(first()).subscribe({
            next: (pl: PlaylistGroup[]) => {
                enqueueSnackbar('Sucessfully downloaded playlist', {variant: 'success'})
                setPlaylist(pl);
            },
            error: (err) => {
                setProgress(false);
                enqueueSnackbar('Failed to download playlist!', {variant: 'error'});
            },
            complete: () => setProgress(false),
        });
    }, [enqueueSnackbar, services]);

    const handleSave = useCallback(() => {
        const filteredPlaylist = viewerRef.current.getFilteredPlaylist();
        if (filteredPlaylist?.length) {
            services.file().save(filteredPlaylist);
        }
    }, [services]);


    const handleFilter = useCallback((filter: string): void => {
        searchChannel.next(filter);
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

    useEffect(() => {
        services.config().getServerConfig().pipe(first()).subscribe({
            next: (cfg: ServerConfig) => {
                setServerConfig(cfg);
            },
            error: (err) => {
                enqueueSnackbar('Failed to download server config!', {variant: 'error'});
            },
            complete: noop,
        });
        return noop
    }, [enqueueSnackbar, services]);


    return (
        <div className="app">
            <div className={'app-header'}>m3u-filter</div>
            <div className={'app-main'}>
                <Sidebar>
                    <ClipboardViewer channel={clipboardChannel}></ClipboardViewer>
                </Sidebar>
                <div className={'app-content'}>
                    <SourceSelector onDownload={handleDownload} serverConfig={serverConfig}/>
                    <PlaylistFilter onFilter={handleFilter}/>
                    <PlaylistViewer ref={viewerRef} playlist={playlist} searchChannel={searchChannel}
                                    onProgress={handleProgress} onCopy={handleOnCopy} onPlay={handleOnPlay}
                                    serverConfig={serverConfig}/>
                    <PlaylistVideo channel={videoChannel}/>
                    <Toolbar onDownload={handleSave}/>
                    <Progress visible={progress}/>
                </div>
            </div>
        </div>
    );
}
