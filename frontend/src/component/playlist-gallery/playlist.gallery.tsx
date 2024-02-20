import ServerConfig from "../../model/server-config";
import {PlaylistGroup, PlaylistItem} from "../../model/playlist";
import './playlist-gallery.scss';
import React, {useCallback, useEffect, useState} from "react";
import {useSnackbar} from "notistack";
import {getIconByName} from "../../icons/icons";
import copyToClipboard from "../../utils/clipboard";
import {first} from "rxjs/operators";
import {noop} from "rxjs";

interface PlaylistGalleryProps {
    serverConfig: ServerConfig;
    data: PlaylistGroup[];
    onCopy: (playlistItem: PlaylistItem) => void;
    onPlay?: (playlistItem: PlaylistItem) => void;
    onDownload?: (playlistItem: PlaylistItem) => void;
    onWebSearch?: (playlistItem: PlaylistItem) => void;

}

export default function PlaylistGallery(props: PlaylistGalleryProps) {

    const {data, onCopy, onPlay, onWebSearch, onDownload, serverConfig} = props;
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();

    const [showcaseGroup, setShowcaseGroup] = useState<PlaylistGroup>();

    useEffect(() => {
        setShowcaseGroup(data.length ? data[0] : undefined)
    }, [data]);

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

    const handleClipboardUrl = useCallback((e: any) => {
        const item = getPlaylistItemById(e.target.dataset.item);
        if (item) {
            onCopy(item);
            copyToClipboard(item.header.url).pipe(first()).subscribe({
                next: value => enqueueSnackbar(value ? "URL copied to clipboard" : "Copy to clipboard failed!", {variant: value ? 'success' : 'error'}),
                error: _ => enqueueSnackbar("Copy to clipboard failed!", {variant: 'error'}),
                complete: noop,
            });
        }
    }, [enqueueSnackbar, getPlaylistItemById, onCopy]);

    const handleWebSearch = useCallback((e: any) => {
        if (onWebSearch) {
            const item = getPlaylistItemById(e.target.dataset.item);
            if (item) {
                onWebSearch(item);
            }
        }
    }, [getPlaylistItemById, onWebSearch]);

    const handleDownloadUrl = useCallback((e: any) => {
        if (onDownload) {
            if (!serverConfig.video.download?.directory) {
                enqueueSnackbar("Please updated the server configuration and add video.download directory and headers!", {variant: 'error'})
            } else {
                const item = getPlaylistItemById(e.target.dataset.item);
                if (item) {
                    onDownload(item);
                }
            }
        }
    }, [serverConfig, enqueueSnackbar, getPlaylistItemById, onDownload]);

    const handlePlayUrl = useCallback((e: any) => {
        if (onPlay) {
            const item = getPlaylistItemById(e.target.dataset.item);
            if (item) {
                onPlay(item);
            }
        }
    }, [onPlay, getPlaylistItemById]);

    const isVideoFile = useCallback((entry: PlaylistItem): boolean => {
        if (serverConfig?.video?.extensions && entry.header.url) {
            for (const ext of serverConfig.video.extensions) {
                if (entry.header.url.endsWith(ext)) {
                    return true;
                }
            }
        }
        return false;
    }, [serverConfig]);

    const handleGroupClick = useCallback((event: any) => {
        const key = parseInt(event.target.dataset.group);
        const group = data.find(plg => plg.id === key);
        setShowcaseGroup(group);
    }, [data]);

    const renderGroup = useCallback((group: PlaylistGroup): React.ReactNode => {
        return <div className={'playlist-gallery__categories-group' + (showcaseGroup?.id === group.id ? ' selected-group' : '') } key={group.id} data-group={group.id}
                    onClick={handleGroupClick}>
            {group.title}
        </div>
    }, [handleGroupClick, showcaseGroup]);

    const renderPlaylistItem = useCallback((playlistItem: PlaylistItem) => {
        return <div className="playlist-gallery__showcase-card" key={playlistItem.id}>
            <div className="playlist-gallery__showcase-card-header">
                {playlistItem.header.name}
            </div>
            <div className="playlist-gallery__showcase-card-content">
                <img  alt="logo" src={playlistItem.header.logo || playlistItem.header.logo_small || 'placeholder.png'}/>
            </div>
            <div className="playlist-gallery__showcase-card-footer">
                <div className={'tool-button'} data-item={playlistItem.id} onClick={handleClipboardUrl}>
                    {getIconByName('LinkRounded')}
                </div>
                <div style={{display: 'none'}} className={'tool-button'} data-item={playlistItem.id}
                     onClick={handlePlayUrl}>
                    {getIconByName('PlayArrow')}
                </div>
                {isVideoFile(playlistItem) &&
                    <>
                        <div className={'tool-button'} data-item={playlistItem.id} onClick={handleDownloadUrl}>
                            {getIconByName('Download')}
                        </div>
                        {serverConfig.video?.web_search &&
                            <div className={'tool-button'} data-item={playlistItem.id} onClick={handleWebSearch}>
                                {getIconByName('WebSearch')}
                            </div>
                        }
                    </>
                }
            </div>
        </div>
    }, [handleClipboardUrl, handleDownloadUrl, handlePlayUrl, handleWebSearch, isVideoFile, serverConfig]);

    const renderShowcase = useCallback(() => {
        if (!showcaseGroup) {
            return <React.Fragment></React.Fragment>
        }
        return showcaseGroup.channels.map(renderPlaylistItem);
    }, [showcaseGroup, renderPlaylistItem]);


    const renderCategories = useCallback(() => {
        if (!data?.length) {
            return <React.Fragment/>;
        }
        return <div className="playlist-gallery__categories">{data.map(renderGroup)}</div>
    }, [data, renderGroup]);

    return <div className="playlist-gallery">
        {renderCategories()}
        <div className="playlist-gallery__showcase">
            {renderShowcase()}
        </div>
    </div>
}
