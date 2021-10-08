import React, {useRef, useState, useCallback} from 'react';
import './app.scss';
import SourceSelector from "../component/source-selector/source-selector";
import PlaylistViewer, {IPlaylistViewer} from "../component/playlist-viewer/playlist-viewer";
import {useSnackbar} from 'notistack';
import Toolbar from "../component/toolbar/toolbar";
import {PlaylistGroup} from "../model/playlist";
import {useServices} from "../provider/service-provider";
import {first} from "rxjs/operators";
import Progress from '../component/progress/progress';

/* eslint-disable @typescript-eslint/no-empty-interface */
interface AppProps {

}

export default function App(props: AppProps) {
    const [progress, setProgress] = useState<boolean>(false);
    const [playlist, setPlaylist] = useState<PlaylistGroup[]>(null);
    const viewerRef = useRef<IPlaylistViewer>();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const services = useServices();
    const handleDownload = useCallback((url: string) => {
        setProgress(true);
        services.playlist().getPlaylist(url).pipe(first()).subscribe({
            next: (playlist: PlaylistGroup[]) => {
                enqueueSnackbar('Sucessfully downloaded playlist', {variant: 'success'})
                setPlaylist(playlist);
            },
            error: (err) => {
                enqueueSnackbar('Failed to download playlist!', {variant: 'error'});
            },
            complete: () => setProgress(false),
        });
    }, [enqueueSnackbar, services]);

    const handleSave = useCallback(() => {
        const filteredPlaylist = viewerRef.current.getFilteredPlaylist();
        if (filteredPlaylist) {
            services.file().save(filteredPlaylist);
        }
    }, [services]);

    return (
        <div className="app">
            <div className={'app-header'}>m3u-filter</div>
            <SourceSelector onDownload={handleDownload}/>
            <PlaylistViewer ref={viewerRef} playlist={playlist}/>
            <Toolbar onDownload={handleSave}/>
            <Progress visible={progress}/>
        </div>
    );
}
