import React, {forwardRef, useImperativeHandle, useMemo, useEffect, useState, useCallback, useRef} from "react";
import './playlist-viewer.scss';
import PlaylistTree, {PlaylistTreeState} from "../playlist-tree/playlist-tree";
import {Observable, noop, tap, finalize} from "rxjs";
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
import {useSnackbar} from "notistack";
import {first} from "rxjs/operators";

function textMatch(text: string, searchRequest: SearchRequest): boolean {
    if (searchRequest.regexp) {
        // eslint-disable-next-line eqeqeq
        return text.match(searchRequest.filter) != undefined;
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

function filterMatchingGroups(playlistCategories: PlaylistCategories, searchRequest: SearchRequest): Observable<PlaylistCategories> {
    return new Observable<PlaylistCategories>((observer) => {
        searchRequest.filter = searchRequest.regexp ? searchRequest.filter :  searchRequest.filter.toLowerCase();
        const result: PlaylistCategories = {
            live:[],
            vod: [],
            series: [],
        };
        for (const category of ['live', 'vod', 'series']) {
            const groups = (playlistCategories as any)[category] ?? [];
            const currentResult = (result as any)[category];
            for (const g of groups) {
                if (textMatch(g.name, searchRequest)) {
                    currentResult.push(g);
                } else {
                    const matches = filterMatchingChannels(g, searchRequest);
                    if (matches) {
                        currentResult.push(matches);
                    }
                }
            }
        }
        if (result.live.length || result.vod.length || result.series.length) {
            observer.next(result);
        } else {
            observer.next(undefined);
        }
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

export default function PlaylistViewer(props:  PlaylistViewerProps) {
    const {serverConfig, playlist, searchChannel,
        onProgress, onCopy, onPlay, onDownload, onWebSearch} = props;
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const [data, setData] = useState<PlaylistCategories>(EmptyPlaylistCategories);
    const [galleryView, setGalleryView] = useState<boolean>(localStorage.getItem("galleryView") === '1');
    const checked = useMemo((): PlaylistTreeState => ({}), []);

    useEffect(() => {
        setData(playlist);
        return noop;
    }, [playlist]);

    useEffect(() => {
        const sub = searchChannel.subscribe((searchRequest: SearchRequest) => {
            let criteria = searchRequest.filter;
            // eslint-disable-next-line eqeqeq
            if (criteria == undefined || !criteria.length || !criteria.trim().length) {
                setData(playlist);
            } else {
                const trimmedCrit = criteria.trim();
                if (trimmedCrit.length < 2) {
                    enqueueSnackbar("Minimum search criteria length is 2", {variant: 'info'});
                } else {
                    searchRequest.filter = trimmedCrit;
                    onProgress && onProgress(true)
                    filterMatchingGroups(playlist, searchRequest).pipe(
                        finalize(() => onProgress && onProgress(false)),
                        first())
                        .subscribe((matches: PlaylistCategories) => {
                            if (matches) {
                                const foundCount = matches.live.length +  matches.vod.length +   matches.series.length;
                                enqueueSnackbar("Found Entries: " + foundCount, {variant: 'success'});
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

    const renderContent = () => {
        if (galleryView) {
            return <PlaylistGallery data={data} onCopy={onCopy} onPlay={onPlay}
                                    onDownload={onDownload}
                                    onWebSearch={onWebSearch}
                                    serverConfig={serverConfig}/>
        }
        return <PlaylistTree data={data} state={checked}
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
}