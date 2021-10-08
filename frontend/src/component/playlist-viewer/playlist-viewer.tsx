import React, {forwardRef, useImperativeHandle, useMemo, useCallback} from "react";
import './playlist-viewer.scss';
import {PlaylistGroup} from "../../model/playlist";
import PlaylistFilter from "../playlist-filter/playlist-filter";
import PlaylistTree, {PlaylistTreeState} from "../playlist-tree/playlist-tree";

function filterPlaylist(playlist: PlaylistGroup[], filter: { [key: string]: boolean }): PlaylistGroup[] {
    if (playlist) {
        return playlist.filter(group => filter[group.id] !== true)
    }
    return null;
}

export interface IPlaylistViewer {
    getFilteredPlaylist: () => PlaylistGroup[];
}

interface PlaylistViewerProps {
    playlist: PlaylistGroup[];
}

const PlaylistViewer = forwardRef<IPlaylistViewer, PlaylistViewerProps>((props: PlaylistViewerProps, ref: any) => {
    const {playlist} = props;
    const checked = useMemo((): PlaylistTreeState => ({}), []);
    const reference = useMemo(() => (
        {
            getFilteredPlaylist: () => filterPlaylist(playlist, checked)
        }), [playlist, checked]);

    useImperativeHandle(ref, () => reference);

    const handleFilter = useCallback((filter: string): void => {
        console.log(filter);
    }, []);

    return <div className={'playlist-viewer'}>
        <PlaylistFilter onFilter={handleFilter}/>
        <PlaylistTree data={playlist} state={checked}/>
    </div>
});

export default PlaylistViewer;