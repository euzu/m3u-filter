import React, {useCallback, useEffect, useState} from 'react';
import './user-playlist.scss';
import {useServices} from "../../provider/service-provider";
import {finalize, first, zip} from 'rxjs';
import {PlaylistCategories, PlaylistCategory} from "../../model/playlist-categories";
import LoadingIndicator from '../loading-indicator/loading-indicator';
import TabSet from "../tab-set/tab-set";
import Checkbox from "../checkbox/checkbox";
import {enqueueSnackbar} from "notistack";
import {getIconByName} from "../../icons/icons";
import PlaylistFilter from "../playlist-filter/playlist-filter";

const CATEGORY_TABS = [
    {label: "Live", key: "live"},
    {label: "VOD", key: "vod"},
    {label: "Series", key: "series"}
];

function isEmpty(value: any): boolean {
    if (value == null) return true;
    if (typeof value === "string") return value.trim() === "";
    if (Array.isArray(value)) return value.length === 0;
    if (typeof value === "object") return Object.keys(value).length === 0;
    return false;
}

const selectEntries = (selected: boolean, list: PlaylistCategory[]): Record<string, boolean> => {
    return list?.reduce((acc: Record<string, boolean>, category: PlaylistCategory) => {
        acc[category.id] = selected;
        return acc;
    }, {});
}

/* eslint-disable @typescript-eslint/no-empty-interface */
interface UserPlaylistProps {

}

export default function UserPlaylist(props: UserPlaylistProps) {
    const services = useServices();
    const [loading, setLoading] = useState(false);
    const [categories, setCategories] = useState<PlaylistCategories>(undefined);
    const [filteredCategories, setFilteredCategories] = useState<PlaylistCategories>({} as any);
    const [selections, setSelections] = useState<Record<string, boolean>>({});
    const [activeTab, setActiveTab] = useState<string>(CATEGORY_TABS[0].key);
    const [showSelected, setShowSelected] = useState<boolean>(false);

    const getActiveCategories =  useCallback((key: string) => {
            let active = ((filteredCategories as any)?.[key] ?? (categories as any)?.[key]) as any;
            if (showSelected) {
                return active.filter((c: PlaylistCategory) => selections[c.id])
            }
            return active;
        },
        [categories, filteredCategories, showSelected]);

    useEffect(() => {
        setLoading(true);
        zip(services.userConfig().getPlaylistBouquet().pipe(first()) ,
            services.userConfig().getPlaylistCategories().pipe(first())).pipe(finalize(() => setLoading(false)))
            .subscribe(([bouquet, categories]: [PlaylistCategories, PlaylistCategories]) => {
                if (isEmpty(bouquet)) {
                    bouquet = undefined;
                }
                if (isEmpty(categories)) {
                    categories = {} as any;
                }
                Object.values(categories).forEach((list: PlaylistCategory[]) => {
                    list.sort((a, b) => a.name.localeCompare(b.name, {sensitivity: 'base'} as any))
                });
                if (bouquet || categories) {
                    const user_bouquet: any = {}
                    Object.values(bouquet ?? categories).filter(Boolean).flat().forEach(c => user_bouquet[c.id] = true);
                    setSelections(user_bouquet);
                }
                setCategories(categories ?? undefined);
            });
    }, [services]);

    const handleCheckboxChange = useCallback((checked:boolean, value: string) => {
        setSelections(selections => ({...selections, [value]: checked}));
    }, []);

    const renderCat = useCallback((cat:PlaylistCategory) => {
        return <div className={'user-playlist__categories__category'} key={cat.id} title={cat.name}>
            <Checkbox label={cat.name}
                      value={cat.id}
                      checked={selections[cat.id]}
                      onSelect={handleCheckboxChange}></Checkbox>
        </div>;
    }, [handleCheckboxChange, selections]);

    const handleTabChange = useCallback((target: string) => {
        setActiveTab(target);
    }, []);

    const handleSave = useCallback(() => {
        setLoading(true);
        const live = categories?.live?.filter(c => selections[c.id]);
        const vod = categories?.vod?.filter(c => selections[c.id]);
        const series = categories?.series?.filter(c => selections[c.id]);
        const bouquet: PlaylistCategories = {live, series, vod};
        services.userConfig().savePlaylistBouquet(bouquet).pipe(first(), finalize(() => setLoading(false))).subscribe({
            next: () => {
                enqueueSnackbar('Successfully save bouquet', {variant: 'success'})
            },
            error: () => {
                enqueueSnackbar('Failed to save bouquet', {variant: 'error'})
            }
        })
    }, [services, categories, selections]);

    const handleSelectAll = useCallback(() => {
        let activeCategories = getActiveCategories(activeTab);
        if (activeCategories?.length) {
            setSelections(selections => ({...selections, ...selectEntries(true, activeCategories)}));
        }
    }, [activeTab, getActiveCategories]);

    const handleDeselectAll = useCallback(() => {
        let activeCategories = getActiveCategories(activeTab);
        if (activeCategories?.length) {
            setSelections(selections => ({...selections, ...selectEntries(false, activeCategories)}));
        }
    }, [activeTab, getActiveCategories]);

    const handleFilter = useCallback((filter: string, regexp: boolean): void => {
        const filtered = (categories as any)?.[activeTab]?.filter((cat: PlaylistCategory) => {
            if (regexp) {
                return cat.name.trim().match(filter) != undefined;
            } else {
                return (cat.name.trim().toLowerCase().indexOf(filter) > -1);
            }
        }) ?? [];
        setFilteredCategories(filteredCategories => ({...filteredCategories, [activeTab]: filtered}));
    }, [categories]);

    const handleShowSelected = useCallback((event: any) => {
        event.target.blur();
        setShowSelected(!showSelected);
    }, [showSelected]);

    return <>
        <LoadingIndicator loading={loading}></LoadingIndicator>
        <div className="user-playlist">
            <div className="user-playlist__toolbar">
                <label>User boutique editor</label>
                <button title={'Save'} onClick={handleSave}>Save</button>
            </div>
            <TabSet tabs={CATEGORY_TABS} active={activeTab} onTabChange={handleTabChange}></TabSet>
            {CATEGORY_TABS.map(tab => <div className={'user-playlist__categories-panel' + (activeTab !== tab.key ? ' hidden' : '')}>
                <div className={'user-playlist__categories__toolbar'}>
                    <div className={'user-playlist__categories__toolbar-filter'}>
                        <PlaylistFilter onFilter={handleFilter}></PlaylistFilter>
                     </div>
                    <button title={'Show selected'} onClick={handleShowSelected} className={showSelected ? 'button-active': ''}>{getIconByName('Checked')}</button>
                    <button title={'Select All'} onClick={handleSelectAll}>{getIconByName('SelectAll')}</button>
                    <button title={'Deselect all'} onClick={handleDeselectAll}>{getIconByName('DeselectAll')}</button>
                </div>
                <div className={'user-playlist__categories'}>
                    {getActiveCategories(tab.key)?.map(renderCat)}
                    </div>
                </div>)
            }
        </div>
    </>;
}
