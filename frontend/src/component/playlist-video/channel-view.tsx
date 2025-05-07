import {PlaylistCategories, PlaylistCategory, PlaylistGroup, PlaylistItem} from "../../model/playlist";
import './channel-view.scss';
import React, {ReactNode, useCallback, useState} from "react";
import {getIconByName} from "../../icons/icons";
import useTranslator from "../../hook/use-translator";
import {set} from "react-datepicker/dist/date_utils";

const getLogo = (item: PlaylistItem): string | undefined => {
    if (item.logo_small && item.logo_small.length > 0) {
        return item.logo_small;
    }
    if (item.logo && item.logo.length > 0) {
        return item.logo;
    }
    return undefined;
}

interface ChannelViewProps {
    data: PlaylistCategories;
    onPlay?: (playlistItem: PlaylistItem) => void;
}

export default function ChannelView(props: ChannelViewProps) {

    const {data, onPlay} = props;
    const translate = useTranslator();
    const [selectedItem, setSelectedItem] = useState<PlaylistItem>();
    const [selectedGroup, setSelectedGroup] = useState<PlaylistGroup>();
    const [selectedCategory, setSelectedCategory] = useState<PlaylistCategory>(PlaylistCategory.LIVE);
    const [openChannelList, setOpenChannelList] = useState<boolean>(true);

    const handleCategorySelect = useCallback((evt: any) => {
        let category = evt.target.dataset.category;
        if ((data as any)?.[category]) {
            setSelectedCategory(category);
        }
    }, [data]);

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

    const handleChannelSelect = useCallback((channel: PlaylistItem) => {
        if (channel) {
            setSelectedItem(channel)
            onPlay?.(channel);
        }
    }, [onPlay]);

    const handleCloseChannelViewState = useCallback(() => {
         setOpenChannelList(false);
    }, []);


    const handleOpenChannelViewState = useCallback(() => {
        if (!openChannelList) {
            setOpenChannelList(true);
        }
    }, [openChannelList]);

    const handleBack = useCallback(() => {
        if (selectedGroup) {
            setSelectedGroup(undefined);
            return;
        }
    }, [selectedGroup]);

    const renderItems = useCallback((category: PlaylistCategory, group: PlaylistGroup): ReactNode => {
        return <div className="channel-view__channels">
            {group.channels.map(channel => (
                <div key={channel.id} onClick={() => handleChannelSelect(channel)}
                     className={'channel-view__channels-channel channel-' + channel.xtream_cluster}>
                    <div className="channel-view__channels-channel-logo">
                        {getLogo(channel) && <img loading='lazy' alt={"logo"} src={getLogo(channel)}
                                                  onError={(e: any) => (e.target.onerror = null) || (e.target.src = "assets/missing-logo.svg")}/>}
                    </div>
                    <div className="channel-view__channels-channel-name">
                        {channel.title}
                    </div>
                </div>
            ))}
        </div>;
    }, [handleChannelSelect]);

    const renderGroups = useCallback((category: PlaylistCategory, groups: PlaylistGroup[]): ReactNode => {
        return <div className="channel-view__groups">
            {groups.map(group => (
                <div key={group.id} onClick={handleGroupSelect}
                     data-category={category} data-group={group.id} className={'channel-view__groups-group'}>
                    {group.name}
                    <span className={'channel-view__groups-group-count'}>({(group.channels?.length ?? 0)})</span>
                </div>
            ))}
        </div>;
    }, [handleGroupSelect]);

    const renderCategories = useCallback(() => {
        return <div className={'channel-view__categories'}>
            {[['live', 'Live'], ['vod', 'VOD'], ['series', 'Series']].map(cat => <div
                key={'channelview-' + cat[0]} onClick={handleCategorySelect} data-category={cat[0]}
                className={'channel-view__categories-category' + ((data as any)?.[cat[0]] ? '' : ' disabled')}>{getIconByName(cat[1])}</div>)}
        </div>;
    }, [data, handleCategorySelect]);

    const renderSelectionContent = useCallback((): ReactNode => {
        // if (selectedItem) {
        //     return renderItem(selectedItem);
        // }
        if (selectedGroup) {
            return renderItems(selectedCategory, selectedGroup);
        }
        if (selectedCategory) {
            return renderGroups(selectedCategory, data[selectedCategory]);
        }
        return <div>{translate("MESSAGES.NO_CONTENT")}</div>
    }, [data, selectedCategory, selectedGroup,
        renderGroups, renderItems, translate])

    const renderContent = useCallback((): ReactNode => {
        if (data) {
            if (selectedItem || selectedGroup || selectedCategory) {
                return <div className="channel-view__content">
                    {renderSelectionContent()}
                </div>;
            }
        }
        return <div>{translate("MESSAGES.NO_CONTENT")}</div>;
    }, [data, selectedCategory, selectedGroup, selectedItem, renderSelectionContent, translate]);

    const renderMenu = useCallback((): ReactNode => {
        return <div className={"channel-view__menu"}>
            <div className={"channel-view__menu-back"}  onClick={handleCloseChannelViewState}>{getIconByName('Clear')}</div>
            {selectedGroup &&
                <div className={"channel-view__menu-back"} onClick={handleBack}>{getIconByName('Back')}</div>
            }
            <div className={"channel-view__menu-title"} onClick={handleBack}>{selectedGroup?.name ?? ''}</div>
        </div>
    }, [selectedGroup, handleBack, handleCloseChannelViewState])

    return <div className={"channel-view" + (openChannelList ? '' : ' channel-view__closed') } onClick={handleOpenChannelViewState}>
        <div className="channel-view__header">
            {renderMenu()}
            {renderCategories()}
        </div>
        {renderContent()}
    </div>
}
