import React, {useCallback, useEffect, useState} from 'react';
import './user-target-playlist.scss';
import {UserPlaylistCategories, UserPlaylistTargetCategories} from "../../model/playlist";
import TabSet from "../tab-set/tab-set";
import Checkbox from "../checkbox/checkbox";
import {getIconByName} from "../../icons/icons";
import PlaylistFilter from "../playlist-filter/playlist-filter";
import useTranslator from "../../hook/use-translator";

const CATEGORY_TABS = [
    {label: "LABEL.LIVE", key: "live"},
    {label: "LABEL.VOD", key: "vod"},
    {label: "LABEL.SERIES", key: "series"}
];

export interface BouquetSelection {
    live: Record<string, boolean>,
    vod: Record<string, boolean>,
    series: Record<string, boolean>,
}

const selectEntries = (selected: boolean, list: string[]): Record<string, boolean> => {
    return list?.reduce((acc: Record<string, boolean>, category: string) => {
        acc[category] = selected;
        return acc;
    }, {});
}

interface UserTargetPlaylistProps {
    visible: boolean;
    categories: UserPlaylistTargetCategories;
    bouquet: UserPlaylistTargetCategories;
    onSelectionChange: (selections: BouquetSelection) => void,
}

export default function UserTargetPlaylist(props: UserTargetPlaylistProps) {
    const {categories, bouquet, visible, onSelectionChange} = props;
    const translate = useTranslator();
    const [filteredCategories, setFilteredCategories] = useState<UserPlaylistCategories>({} as any);
    const [selections, setSelections] = useState<BouquetSelection>({} as any);
    const [activeTab, setActiveTab] = useState<string>(CATEGORY_TABS[0].key);
    const [showSelected, setShowSelected] = useState<boolean>(false);

    useEffect(() => {
        if (categories) {
            Object.values(categories).forEach((list: string[]) => {
                list.sort((a, b) => a.localeCompare(b, {sensitivity: 'base'} as any))
            });
            const mapToSelection = (userBouquet: any, cluster: string) => {
                let clusterBouquet = userBouquet?.[cluster];
                return (categories as any)[cluster].reduce((acc: any, e: string) => {
                    acc[e] = clusterBouquet ? clusterBouquet?.indexOf(e) >= 0  : true
                    return acc;
                }, {})
            }
            const user_bouquets = {
                live: mapToSelection(bouquet, 'live'),
                vod: mapToSelection(bouquet, 'vod'),
                series: mapToSelection(bouquet,'series'),
            };
            setSelections(user_bouquets);
        }
    }, [bouquet, categories])

    const getActiveCategories = useCallback((key: string) => {
            let active = ((filteredCategories as any)?.[key] ?? (categories as any)?.[key]) as any;
            if (showSelected) {
                return active.filter((c: string) => (selections as any)[key]?.[c])
            }
            return active;
        },
        [categories, filteredCategories, showSelected, selections]);


    const handleCheckboxChange = useCallback((value: string, checked: boolean) => {
        let clusterSelections = (selections as any)[activeTab] as any;
        if (!clusterSelections) {
            clusterSelections = {};
            (selections as any)[activeTab] = clusterSelections as any;
        }
        clusterSelections[value] = checked;
        let newSelections = ({...selections, [activeTab]: clusterSelections});
        onSelectionChange(newSelections)
        setSelections(newSelections);
    }, [activeTab, selections, onSelectionChange]);

    const renderCat = useCallback((cat: string) => {
        const clusterSelections = (selections as any)[activeTab];
        // eslint-disable-next-line eqeqeq
        const selected =  clusterSelections?.[cat] === true;
        return <div className={'user-target-playlist__categories__category'} key={cat} data-tooltip={cat}>
            <Checkbox label={cat}
                      value={cat}
                      checked={selected}
                      onSelect={handleCheckboxChange}></Checkbox>
        </div>;
    }, [handleCheckboxChange, selections, activeTab]);

    const toggleSelection = (selected: boolean) => {
        let activeCategories = getActiveCategories(activeTab);
        if (activeCategories?.length) {
            const newSelections: any = {...selections, [activeTab]: selectEntries(selected, activeCategories)};
            onSelectionChange(newSelections);
            setSelections(newSelections);
        }
    };

    const handleFilter = useCallback((filter: string, regexp: boolean): void => {
        let filter_value = regexp ? filter : filter.toLowerCase();
        if (filter_value?.length) {
            const filtered = (categories as any)?.[activeTab]?.filter((cat: string) => {
                if (regexp) {
                    // eslint-disable-next-line eqeqeq
                    return cat.trim().match(filter_value) != undefined;
                } else {
                    return (cat.trim().toLowerCase().indexOf(filter_value) > -1);
                }
            }) ?? [];
            setFilteredCategories(filteredCategories => ({...filteredCategories, [activeTab]: filtered}));
        } else {
            setFilteredCategories(filteredCategories => ({
                ...filteredCategories,
                [activeTab]: (categories as any)?.[activeTab]
            }));
        }
    }, [activeTab, categories]);

    const handleShowSelected = useCallback((event: any) => {
        event.target.blur();
        setShowSelected(!showSelected);
    }, [showSelected]);

    const tabs = CATEGORY_TABS.filter(tab => (categories as any)?.[tab.key]?.length);
    if (tabs.length === 0) {
        return <div className={"user-target-playlist" + (visible ? '' : ' hidden')}>
            {translate("MESSAGES.NO_CONTENT")}
        </div>
    }
    return <>
        <div className={"user-target-playlist" + (visible ? '' : ' hidden')}>
            <TabSet tabs={tabs} active={activeTab} onTabChange={setActiveTab}></TabSet>
            {tabs.map(tab => <div key={tab.key}
                                  className={'user-target-playlist__categories-panel' + (activeTab !== tab.key ? ' hidden' : '')}>
                <div className={'user-target-playlist__categories__toolbar'}>
                    <div className={'user-target-playlist__categories__toolbar-filter'}>
                        <PlaylistFilter onFilter={handleFilter}></PlaylistFilter>
                    </div>
                    <button className={showSelected ? 'button-active' : ''} data-tooltip='LABEL.SHOW_SELECTED'
                            onClick={handleShowSelected}>{getIconByName('Checked')}</button>
                    <button data-tooltip='LABEL.SELECT_ALL'
                            onClick={() => toggleSelection(true)}>{getIconByName('SelectAll')}</button>
                    <button data-tooltip='LABEL.DESELECT_ALL'
                            onClick={() => toggleSelection(false)}>{getIconByName('DeselectAll')}</button>
                </div>
                <div className={'user-target-playlist__categories'}>
                    {getActiveCategories(tab.key)?.map(renderCat)}
                </div>
            </div>)
            }
        </div>
    </>;
}
