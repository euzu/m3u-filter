import React, {useRef, useState, useCallback, useMemo} from 'react';
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
import {Subject} from "rxjs";
import PlaylistVideo from "../component/playlist-video/playlist-video";

/* eslint-disable @typescript-eslint/no-empty-interface */
interface AppProps {

}

export default function App(props: AppProps) {
    const searchChannel = useMemo<Subject<string>>(() => new Subject<string>(), []);
    const [progress, setProgress] = useState<boolean>(false);
    const [playlist, setPlaylist] = useState<PlaylistGroup[]>([]);
    const viewerRef = useRef<IPlaylistViewer>();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const services = useServices();
    const videoChannel = useMemo(() => new Subject<PlaylistItem>(), []);
    const handleDownload = useCallback((url: string) => {
        setProgress(true);
        services.playlist().getPlaylist(url).pipe(first()).subscribe({
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
        if (filteredPlaylist) {
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

    return (
        <div className="app">
            <div className={'app-header'}>m3u-filter</div>
            <SourceSelector onDownload={handleDownload}/>
            <PlaylistFilter onFilter={handleFilter}/>
            <PlaylistViewer ref={viewerRef} playlist={playlist} searchChannel={searchChannel} onProgress={handleProgress} onPlay={handleOnPlay}/>
            <PlaylistVideo channel={videoChannel}/>
            <Toolbar onDownload={handleSave}/>
            <Progress visible={progress}/>
        </div>
    );
}
