import ServerConfig from "../../model/server-config";
import {PlaylistCategories, PlaylistCategory, PlaylistGroup, PlaylistItem, XtreamCluster} from "../../model/playlist";
import './playlist-gallery.scss';
import React, {ReactNode, useCallback, useEffect, useState} from "react";
import {useSnackbar} from "notistack";
import {getIconByName} from "../../icons/icons";
import useTranslator from "../../hook/use-translator";

const getLogo = (item: PlaylistItem): string | undefined => {
    if (item.logo_small && item.logo_small.length > 0) {
        return item.logo_small;
    }
    if (item.logo && item.logo.length > 0) {
        return item.logo;
    }
    return undefined;
}

interface PlaylistGalleryProps {
    serverConfig: ServerConfig;
    data: PlaylistCategories;
    onCopy: (playlistItem: PlaylistItem) => void;
    onPlay?: (playlistItem: PlaylistItem) => void;
    onDownload?: (playlistItem: PlaylistItem) => void;
    onWebSearch?: (playlistItem: PlaylistItem) => void;

}

export default function PlaylistGallery(props: PlaylistGalleryProps) {

    const {data, onCopy, onPlay, onWebSearch, onDownload, serverConfig} = props;
    // const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const translate = useTranslator();
    const [selectedItem, setSelectedItem] = useState<PlaylistItem>();
    const [selectedGroup, setSelectedGroup] = useState<PlaylistGroup>();
    const [selectedCategory, setSelectedCategory] = useState<PlaylistCategory>();

    // useEffect(() => {
    //     if (category) {
    //         setGroup((data as any)?.[category])
    //     }
    // }, [data, category]);
    //
    // const getPlaylistItemById = useCallback((itemId: string): PlaylistItem => {
    //     // const id = parseInt(itemId);
    //     // if (data && !isNaN(id)) {
    //     //     for (let i = 0, len = data.length; i < len; i++) {
    //     //         const group = data[i];
    //     //         for (let j = 0, clen = group.channels.length; j < clen; j++) {
    //     //             const plitem = group.channels[j];
    //     //             // eslint-disable-next-line eqeqeq
    //     //             if (plitem.id == id) {
    //     //                 return plitem;
    //     //             }
    //     //         }
    //     //     }
    //     // }
    //     return undefined;
    // }, [data]);
    //
    // const handleClipboardUrl = useCallback((e: any) => {
    //     const item = getPlaylistItemById(e.target.dataset.item);
    //     if (item) {
    //         onCopy(item);
    //         copyToClipboard(item.header.url).pipe(first()).subscribe({
    //             next: value => enqueueSnackbar(value ? "URL copied to clipboard" : "Copy to clipboard failed!", {variant: value ? 'success' : 'error'}),
    //             error: _ => enqueueSnackbar("Copy to clipboard failed!", {variant: 'error'}),
    //             complete: noop,
    //         });
    //     }
    // }, [enqueueSnackbar, getPlaylistItemById, onCopy]);

    // const handleWebSearch = useCallback((e: any) => {
    //     if (onWebSearch) {
    //         const item = getPlaylistItemById(e.target.dataset.item);
    //         if (item) {
    //             onWebSearch(item);
    //         }
    //     }
    // }, [getPlaylistItemById, onWebSearch]);

    // const handleDownloadUrl = useCallback((e: any) => {
    //     if (onDownload) {
    //         if (!serverConfig.video.download?.directory) {
    //             enqueueSnackbar("Please updated the server configuration and add video.download directory and headers!", {variant: 'error'})
    //         } else {
    //             const item = getPlaylistItemById(e.target.dataset.item);
    //             if (item) {
    //                 onDownload(item);
    //             }
    //         }
    //     }
    // }, [serverConfig, enqueueSnackbar, getPlaylistItemById, onDownload]);

    // const handlePlayUrl = useCallback((e: any) => {
    //     if (onPlay) {
    //         const item = getPlaylistItemById(e.target.dataset.item);
    //         if (item) {
    //             onPlay(item);
    //         }
    //     }
    // }, [onPlay, getPlaylistItemById]);
    //
    // const isVideoFile = useCallback((entry: PlaylistItem): boolean => {
    //     if (serverConfig?.video?.extensions && entry.header.url) {
    //         for (const ext of serverConfig.video.extensions) {
    //             if (entry.header.url.endsWith(ext)) {
    //                 return true;
    //             }
    //         }
    //     }
    //     return false;
    // }, [serverConfig]);

    // const handleGroupClick = useCallback((event: any) => {
    //     // const key = parseInt(event.target.dataset.group);
    //     // const group = data.find(plg => plg.id === key);
    //     // setShowcaseGroup(group);
    // }, [data]);
    //
    // const renderGroup = useCallback((group: PlaylistGroup): React.ReactNode => {
    //     return <div className={'playlist-gallery__categories-group' + (group?.id === group.id ? ' selected-group' : '') } key={group.id} data-group={group.id}
    //                 onClick={handleGroupClick}>
    //         {group.name}
    //     </div>
    // }, [handleGroupClick, group]);
    //
    // const renderPlaylistItem = useCallback((playlistItem: PlaylistItem) => {
    //     return <div className="playlist-gallery__showcase-card" key={playlistItem.id}>
    //         <div className="playlist-gallery__showcase-card-header">
    //             {playlistItem.header.name}
    //         </div>
    //         <div className="playlist-gallery__showcase-card-content">
    //             <img alt="logo" src={playlistItem.header.logo || playlistItem.header.logo_small || 'assets/placeholder.png'}/>
    //         </div>
    //         <div className="playlist-gallery__showcase-card-footer">
    //             <div className={'tool-button'} data-item={playlistItem.id} onClick={handleClipboardUrl}>
    //                 {getIconByName('LinkRounded')}
    //             </div>
    //             <div style={{display: 'none'}} className={'tool-button'} data-item={playlistItem.id}
    //                  onClick={handlePlayUrl}>
    //                 {getIconByName('PlayArrow')}
    //             </div>
    //             {isVideoFile(playlistItem) &&
    //                 <>
    //                     <div className={'tool-button'} data-item={playlistItem.id} onClick={handleDownloadUrl}>
    //                         {getIconByName('Download')}
    //                     </div>
    //                     {serverConfig.video?.web_search &&
    //                         <div className={'tool-button'} data-item={playlistItem.id} onClick={handleWebSearch}>
    //                             {getIconByName('WebSearch')}
    //                         </div>
    //                     }
    //                 </>
    //             }
    //         </div>
    //     </div>
    // }, [handleClipboardUrl, handleDownloadUrl, handlePlayUrl, handleWebSearch, isVideoFile, serverConfig]);

    // const renderShowcase = useCallback(() => {
    //     if (!group) {
    //         return <React.Fragment></React.Fragment>
    //     }
    //     return group.channels?.map(renderPlaylistItem);
    // }, [group, renderPlaylistItem]);

    // useEffect(() => {
    //     if (selectedItem) {
    //
    //     }
    // }, [selectedItem]);

    useEffect(() => {
        if (selectedCategory) {
            let groups = (data as any)[selectedCategory];
            if (!groups?.length) {
                setSelectedItem(undefined);
                setSelectedGroup(undefined);
                setSelectedCategory(undefined);
            }
            if (selectedGroup && groups?.length) {
                let group = groups.find((g: PlaylistGroup) => g.id === selectedGroup.id);
                if (!group) {
                    setSelectedItem(undefined);
                    setSelectedGroup(undefined);
                }
                if (group && selectedItem) {
                    let channel = group.channels.find((c: PlaylistItem) => c.id === selectedItem.id);
                    if (!channel) {
                        setSelectedItem(undefined);
                    }
                }
            }
        }
    }, [data, selectedCategory, selectedGroup, selectedItem])

    const handleCategorySelect = useCallback((evt: any) => {
        let category = evt.target.dataset.category;
        setSelectedCategory(category);
    }, []);

    const handleGroupSelect = useCallback((evt: any) => {
        let category = evt.target.dataset.category;
        let groupId = evt.target.dataset.group;
        let groups: PlaylistGroup[] = (data as any)[category];
        if (groups) {
            // eslint-disable-next-line eqeqeq
            let group = groups.find(grp => grp.id == groupId);
            setSelectedGroup(group)
        }
    }, [data]);

    const handleChannelSelect = useCallback((evt: any) => {
        let category = evt.target.dataset.category;
        if (category !== PlaylistCategory.LIVE) {
            let groupId = evt.target.dataset.group;
            let channelId = evt.target.dataset.channel;
            let groups: PlaylistGroup[] = (data as any)[category];
            if (groups) {
                // eslint-disable-next-line eqeqeq
                let group = groups.find(grp => grp.id == groupId);
                if (group) {
                    // eslint-disable-next-line eqeqeq
                    let channel = group.channels.find(channel => channel.id == channelId);
                    setSelectedItem(channel)
                }
            }
        }
    }, [data]);

    const handleBack = useCallback(() => {
        if (selectedItem) {
            setSelectedItem(undefined);
            return;
        }
        if (selectedGroup) {
            setSelectedGroup(undefined);
            return;
        }
        if (selectedCategory) {
            setSelectedCategory(undefined);
            return;
        }
    }, [selectedCategory, selectedGroup, selectedItem]);


    const renderItem = useCallback((item: PlaylistItem): ReactNode => {
        if (item.xtream_cluster === XtreamCluster.Video) {
            return <div onClick={() => onPlay?.(item)} className="playlist-gallery">
                {JSON.stringify(item)}
                <div>{item.title}</div>
                <img loading='lazy'  src={item.logo} alt={'logo'} />
            </div>;

        }
        return <div>{JSON.stringify(item)}</div>;
    }, []);

    const renderItems = useCallback((category: PlaylistCategory, group: PlaylistGroup): ReactNode => {
        return <div className="playlist-gallery__channels">
            {group.channels.map(channel => (
                <div key={channel.id} onClick={handleChannelSelect}
                     data-category={category} data-group={group.id} data-channel={channel.id}
                     className={'playlist-gallery__channels-channel channel-' + channel.xtream_cluster}>
                    <div className="playlist-gallery__channels-channel-logo">
                        {getLogo(channel) && <img loading='lazy' alt={"logo"} src={getLogo(channel)}
                                                  onError={(e: any) => (e.target.onerror=null) || (e.target.src="assets/missing-logo.svg")}/>}
                    </div>
                    <div className="playlist-gallery__channels-channel-name">
                        {channel.title}
                    </div>
                </div>
            ))}
        </div>;
    }, [handleChannelSelect]);

    const renderGroups = useCallback((category: PlaylistCategory, groups: PlaylistGroup[]): ReactNode => {
        return <div className="playlist-gallery__groups">
            {groups.map(group => (
                <div key={group.id} onClick={handleGroupSelect}
                     data-category={category} data-group={group.id} className={'playlist-gallery__groups-group'}>
                    {group.name}
                </div>
            ))}
        </div>;
    }, [handleGroupSelect]);

    const renderCategories = useCallback((categories: PlaylistCategories) => {
        const newNode = (tooltip: string, cat: string, icon: string) => {
            return <div key={cat} onClick={handleCategorySelect} data-tooltip={translate(tooltip)} data-category={cat}
                        className={'playlist-gallery__category'}>{getIconByName(icon)}</div>;
        };
        let nodes = [];
        if (categories.live?.length) {
            nodes.push(newNode('LABEL.LIVE', 'live', 'Live'));
        }
        if (categories.vod?.length) {
            nodes.push(newNode('LABEL.VOD', 'vod', 'VOD'));
        }
        if (categories.series?.length) {
            nodes.push(newNode('LABEL.SERIES', 'series', 'Series'));
        }
        if (nodes.length) {
            return <div className="playlist-gallery__categories">{nodes}</div>
        }
        return <div>{translate("MESSAGES.NO_CONTENT")}</div>
    }, [translate, handleCategorySelect]);

    const renderSelectionContent = useCallback((): ReactNode => {
        if (selectedItem) {
            return renderItem(selectedItem);
        }
        if (selectedGroup) {
            return renderItems(selectedCategory, selectedGroup);
        }
        if (selectedCategory) {
            return renderGroups(selectedCategory, data[selectedCategory]);
        }
        return <div>{translate("MESSAGES.NO_CONTENT")}</div>
    }, [data, selectedCategory, selectedGroup, selectedItem,
        renderGroups, renderItems, renderItem, translate])

    const renderContent = useCallback((): ReactNode => {
        if (data) {
            if (selectedItem || selectedGroup || selectedCategory) {
                return <div className="playlist-gallery__content">
                    {renderSelectionContent()}
                </div>;
            }
            if (data) {
                return renderCategories(data);
            }
        }
        return <div>{translate("MESSAGES.NO_CONTENT")}</div>;
    }, [data, selectedCategory, selectedGroup, selectedItem, renderCategories, renderSelectionContent, translate]);

    const handleBreadcrumb = useCallback((evt: any) => {
        const index = evt.target.dataset.index;
        // eslint-disable-next-line eqeqeq
        if (index != undefined) {
            switch (index) {
                case '0': {
                    setSelectedGroup(undefined);
                    setSelectedItem(undefined);
                    break;
                }
                case '1': {
                    setSelectedItem(undefined);
                    break;
                }
            }
        }
    }, []);

    const renderBreadcrumbs = useCallback((): ReactNode => {
        const crumbs = [selectedCategory, selectedGroup?.name, selectedItem?.title].filter(Boolean);
        if (crumbs.length) {
            return <div className="playlist-gallery__breadcrumbs">
                <button onClick={handleBack}
                        className="playlist-gallery__groups-toolbar-item">{getIconByName('Back')}</button>
                {crumbs.map((b, index) => <span key={b} data-index={index} onClick={handleBreadcrumb}>{b}</span>)}
            </div>
        }
        return <></>
    }, [selectedCategory, selectedGroup, selectedItem, handleBack, handleBreadcrumb])

    return <div className="playlist-gallery">
        {renderBreadcrumbs()}
        {renderContent()}
    </div>
}
