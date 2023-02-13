import React, {useCallback, useState, useRef} from 'react';
import './playlist-tree.scss';
import {PlaylistGroup, PlaylistItem} from "../../model/playlist";
import {ExpandMore, ChevronRight, LinkRounded, PlayArrow} from "@mui/icons-material";
import copyToClipboard from "../../utils/clipboard";
import {first} from "rxjs/operators";
import {noop} from "rxjs";
import {useSnackbar} from "notistack";

export type PlaylistTreeState = { [key: number]: boolean };

interface PlaylistTreeProps {
    data: PlaylistGroup[];
    state: PlaylistTreeState;
    onCopy: (playlistItem: PlaylistItem) => void;
    onPlay?: (playlistItem: PlaylistItem) => void;
}

export default function PlaylistTree(props: PlaylistTreeProps) {
    const {state, data, onCopy, onPlay} = props;

    const [, setForceUpdate] = useState(null);
    const expanded = useRef<PlaylistTreeState>({});
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();

    const getPlaylistItemById = useCallback((itemId: string): PlaylistItem => {
        const id = parseInt(itemId);
        if (data && !isNaN(id)) {
            for (let i=0, len = data.length; i < len; i++) {
                const group = data[i];
                for (let j=0, clen = group.channels.length; j < clen; j++) {
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
            copyToClipboard(item.url).pipe(first()).subscribe({
                next: value => enqueueSnackbar(value ? "URL copied to clipboard" : "Copy to clipboard failed!", {variant: value ? 'success' : 'error'}),
                error: err => enqueueSnackbar("Copy to clipboard failed!", {variant: 'error'}),
                complete: noop,
            });
        }
    }, [enqueueSnackbar, getPlaylistItemById, onCopy]);

    const handlePlayUrl = useCallback((e: any) => {
        if (onPlay) {
            const item = getPlaylistItemById(e.target.dataset.item);
            if (item) {
               onPlay(item);
            }
        }
    }, [onPlay, getPlaylistItemById]);

    const renderEntry = useCallback((entry: PlaylistItem, index: number): React.ReactNode => {
        return <div key={entry.id} className={'tree-channel'}>
            <div className={'tree-channel-tools'}>
                <div className={'tool-button'} data-item={entry.id} onClick={handleClipboardUrl}>
                    <LinkRounded/>
                </div>
                <div style={{display: 'none'}} className={'tool-button'} data-item={entry.id} onClick={handlePlayUrl}>
                    <PlayArrow/>
                </div>
            </div>
            <div className={'tree-channel-content'}>
                <div className={'tree-channel-nr'}>{index + 1}</div>
                {entry.header.name}</div>
        </div>
    }, [handleClipboardUrl, handlePlayUrl]);

    const renderGroup = useCallback((group: PlaylistGroup): React.ReactNode => {
        return <div className={'tree-group'} key={group.id}>
            <div className={'tree-group-header'}>
                <div className={'tree-expander'} data-group={group.id}
                     onClick={handleExpand}>{expanded.current[group.id] ? <ExpandMore/> : <ChevronRight/>}</div>
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

    return <div className={'playlist-tree'}>{renderPlaylist()}</div>;
} 