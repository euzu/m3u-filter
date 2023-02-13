import React, {forwardRef, useImperativeHandle, useMemo, useEffect, useState} from "react";
import './playlist-viewer.scss';
import {PlaylistGroup, PlaylistItem} from "../../model/playlist";
import PlaylistTree, {PlaylistTreeState} from "../playlist-tree/playlist-tree";
import {Observable, noop, tap, finalize} from "rxjs";
import {useSnackbar} from "notistack";
import {first} from "rxjs/operators";

function filterPlaylist(playlist: PlaylistGroup[], filter: { [key: string]: boolean }): PlaylistGroup[] {
    if (playlist) {
        return playlist.filter(group => filter[group.id] !== true)
    }
    return null;
}

function textMatch(text: string, criteria: string): boolean {
    return (text.toLowerCase().indexOf(criteria) > -1);
}

function filterMatchingChannels(grp: PlaylistGroup, criteria: string): PlaylistGroup {
    let channels: PlaylistItem[] = [];
    for (const c of grp.channels) {
        if (textMatch(c.header.name, criteria)) {
            channels.push(c);
        }
    }
    if (channels.length) {
        return {
            id: grp.id,
            title: grp.title,
            channels
        } as PlaylistGroup;
    }
    return undefined;
}

function filterMatchingGroups(gl: PlaylistGroup[], criteria: string): Observable<PlaylistGroup[]> {
    return new Observable<PlaylistGroup[]>((observer) => {
        const lcrit = criteria.toLowerCase();
        const result: PlaylistGroup[] = [];
        for (const g of gl) {
            if (textMatch(g.title, lcrit)) {
                result.push(g);
            } else {
                const matches = filterMatchingChannels(g, lcrit);
                if (matches) {
                    result.push(matches);
                }
            }
        }
        observer.next(result);
        observer.complete();
    })
}

export interface IPlaylistViewer {
    getFilteredPlaylist: () => PlaylistGroup[];
}

interface PlaylistViewerProps {
    playlist: PlaylistGroup[];
    searchChannel: Observable<string>;
    onProgress: (value: boolean) => void;
    onCopy: (playlistItem: PlaylistItem) => void;
    onPlay?: (playlistItem: PlaylistItem) => void;
}

const PlaylistViewer = forwardRef<IPlaylistViewer, PlaylistViewerProps>((props: PlaylistViewerProps, ref: any) => {
    const {playlist, searchChannel, onProgress, onCopy, onPlay} = props;
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const [data, setData] = useState<PlaylistGroup[]>([]);
    const checked = useMemo((): PlaylistTreeState => ({}), []);
    const reference = useMemo(() => (
        {
            getFilteredPlaylist: () => filterPlaylist(playlist, checked)
        }), [playlist, checked]);

    useImperativeHandle(ref, () => reference);

    useEffect(() => {
        setData(playlist);
        return noop;
    }, [playlist]);

    useEffect(() => {
        const sub = searchChannel.subscribe((criteria: string) => {
            if (criteria == null || !criteria.length || !criteria.trim().length) {
                setData(playlist);
            } else {
                const trimmedCrit = criteria.trim();
                if (trimmedCrit.length < 2) {
                    enqueueSnackbar("Minimum search criteria length is 2", {variant: 'info'});
                } else {
                    filterMatchingGroups(playlist, trimmedCrit).pipe(
                        tap(() => onProgress && onProgress(true)),
                        finalize(() => onProgress && onProgress(false)),
                        first())
                        .subscribe((matches: PlaylistGroup[]) => {
                            if (matches.length) {
                                setData(matches);
                            } else {
                                enqueueSnackbar("Nothing found!", {variant: 'info'});
                            }
                        });
                }
            }
        });
        return () => sub.unsubscribe();
    }, [searchChannel, playlist, enqueueSnackbar, onProgress]);

    return <div className={'playlist-viewer'}>
        <PlaylistTree data={data} state={checked} onCopy={onCopy} onPlay={onPlay}/>
    </div>
});

export default PlaylistViewer;