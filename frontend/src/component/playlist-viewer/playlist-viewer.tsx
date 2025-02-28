import React, {forwardRef, useImperativeHandle, useMemo, useEffect, useState, useCallback, useRef} from "react";
import './playlist-viewer.scss';
import PlaylistTree, {PlaylistTreeState} from "../playlist-tree/playlist-tree";
import {Observable, noop} from "rxjs";
import ServerConfig from "../../model/server-config";
import PlaylistGallery from "../playlist-gallery/playlist-gallery";
import {getIconByName} from "../../icons/icons";
import {
    EmptyPlaylistCategories,
    PlaylistCategories,
    PlaylistCategory,
    PlaylistGroup,
    PlaylistItem
} from "../../model/playlist";

function filterPlaylist(playlist: PlaylistGroup[], filter: { [key: string]: boolean }): PlaylistGroup[] {
    if (playlist) {
        return playlist.filter(group => filter[group.id] !== true)
    }
    return undefined;
}

function textMatch(text: string, searchRequest: SearchRequest): boolean {
    if (searchRequest.regexp) {
        // eslint-disable-next-line eqeqeq
        return text.toLowerCase().match(searchRequest.filter) != undefined;
    } else {
        return (text.toLowerCase().indexOf(searchRequest.filter) > -1);
    }
}

function filterMatchingChannels(grp: PlaylistGroup, searchRequest: SearchRequest): PlaylistGroup {
    let channels: PlaylistItem[] = [];
    for (const c of grp.channels) {
        if (textMatch(c.name, searchRequest)) {
            channels.push(c);
        }
    }
    if (channels.length) {
        return {
            id: grp.id,
            name: grp.name,
            channels
        } as PlaylistGroup;
    }
    return undefined;
}

function filterMatchingGroups(gl: PlaylistGroup[], searchRequest: SearchRequest): Observable<PlaylistGroup[]> {
    return new Observable<PlaylistGroup[]>((observer) => {
        searchRequest.filter = searchRequest.filter.toLowerCase();
        const result: PlaylistGroup[] = [];
        for (const g of gl) {
            if (textMatch(g.name, searchRequest)) {
                result.push(g);
            } else {
                const matches = filterMatchingChannels(g, searchRequest);
                if (matches) {
                    result.push(matches);
                }
            }
        }
        observer.next(result);
        observer.complete();
    })
}

export interface SearchRequest  {
    filter: string,
    regexp: boolean
}


export interface IPlaylistViewer {
    getFilteredPlaylist: () => PlaylistCategories;
}

interface PlaylistViewerProps {
    serverConfig: ServerConfig;
    playlist: PlaylistCategories;
    searchChannel: Observable<SearchRequest>;
    onProgress: (value: boolean) => void;
    onCopy: (playlistItem: PlaylistItem) => void;
    onPlay?: (playlistItem: PlaylistItem) => void;
    onDownload?: (playlistItem: PlaylistItem) => void;
    onWebSearch?: (playlistItem: PlaylistItem) => void;
}

const PlaylistViewer = forwardRef<IPlaylistViewer, PlaylistViewerProps>((props: PlaylistViewerProps, ref: any) => {
    const {serverConfig, playlist, searchChannel,
        onProgress, onCopy, onPlay, onDownload, onWebSearch} = props;
    // const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const [data, setData] = useState<PlaylistCategories>(EmptyPlaylistCategories);
    const currentCategoryRef = useRef<PlaylistCategory>(PlaylistCategory.LIVE);
    const [galleryView, setGalleryView] = useState<boolean>(localStorage.getItem("galleryView") === '1');
    const checked = useMemo((): PlaylistTreeState => ({}), []);
    const reference = useMemo(() => (
        {
            getFilteredPlaylist: () => filterPlaylist((playlist as any)[currentCategoryRef.current], checked)
        }), [playlist, checked]);

    useImperativeHandle(ref, () => reference);

    useEffect(() => {
        setData(playlist);
        return noop;
    }, [playlist]);

    // useEffect(() => {
    //     const sub = searchChannel.subscribe((searchRequest: SearchRequest) => {
    //         let criteria = searchRequest.filter;
    //         // eslint-disable-next-line eqeqeq
    //         if (criteria == undefined || !criteria.length || !criteria.trim().length) {
    //             setData(playlist);
    //         } else {
    //             const trimmedCrit = criteria.trim();
    //             if (trimmedCrit.length < 2) {
    //                 enqueueSnackbar("Minimum search criteria length is 2", {variant: 'info'});
    //             } else {
    //                 searchRequest.filter = trimmedCrit;
    //                 filterMatchingGroups(playlist[currentCatgoryRef.current], searchRequest).pipe(
    //                     tap(() => onProgress && onProgress(true)),
    //                     finalize(() => onProgress && onProgress(false)),
    //                     first())
    //                     .subscribe((matches: PlaylistGroup[]) => {
    //                         if (matches.length) {
    //                             setData(matches);
    //                         } else {
    //                             enqueueSnackbar("Nothing found!", {variant: 'info'});
    //                         }
    //                     });
    //             }
    //         }
    //     });
    //     return () => sub.unsubscribe();
    // }, [searchChannel, playlist, enqueueSnackbar, onProgress]);

    const renderContent = () => {
        if (galleryView) {
            return <PlaylistGallery data={data} onCopy={onCopy} onPlay={onPlay}
                                    onDownload={onDownload}
                                    onWebSearch={onWebSearch}
                                    serverConfig={serverConfig}/>
        }
        return <PlaylistTree data={data[currentCategoryRef.current]} state={checked}
                             onCopy={onCopy} onPlay={onPlay}
                             onDownload={onDownload}
                             onWebSearch={onWebSearch}
                             serverConfig={serverConfig}/>
    }

    const toggleView = useCallback(() => {
        setGalleryView(data => {
            localStorage.setItem("galleryView", data ? '0' : '1');
            return !data;
        });
    }, []);

    return <div className={'playlist-viewer'}>
        <div className={'playlist-viewer__header'}>
            <div className={'tool-button'} onClick={toggleView}>
                {getIconByName('Gallery')}
            </div>
        </div>
        <div className={'playlist-viewer__content'}>
            {renderContent()}
        </div>
    </div>
});

export default PlaylistViewer;