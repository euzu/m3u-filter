import React, {useCallback, useEffect, useMemo, useState} from 'react';
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
import useTranslator from "../../hook/use-translator";

const CATEGORY_TABS = [
    {label: "LABEL.LIVE", key: "live"},
    {label: "LABEL.VOD", key: "vod"},
    {label: "LABEL.SERIES", key: "series"}
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
    const translate = useTranslator();
    const [loading, setLoading] = useState(false);
    const [categories, setCategories] = useState<PlaylistCategories>(undefined);
    const [filteredCategories, setFilteredCategories] = useState<PlaylistCategories>({} as any);
    const [selections, setSelections] = useState<Record<string, boolean>>({});
    const [activeTab, setActiveTab] = useState<string>(CATEGORY_TABS[0].key);
    const [showSelected, setShowSelected] = useState<boolean>(false);
    const tabs = useMemo(() => CATEGORY_TABS.map(d => ({key: d.key, label: translate(d.label) })), [translate])

    const getActiveCategories =  useCallback((key: string) => {
            let active = ((filteredCategories as any)?.[key] ?? (categories as any)?.[key]) as any;
            if (showSelected) {
                return active.filter((c: PlaylistCategory) => selections[c.id])
            }
            return active;
        },
        [categories, filteredCategories, showSelected, selections]);

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
                    CATEGORY_TABS.map(t => t.key).forEach(key => {
                        let current_bouquet: PlaylistCategory[] = ((bouquet as any)?.[key]?.length ?  (bouquet as any)[key] : (categories as any)?.[key]) ?? [];
                        Object.values(current_bouquet).forEach((c: PlaylistCategory) => user_bouquet[c.id] = true);
                    });
                    setSelections(user_bouquet);
                }
                setCategories(categories ?? undefined);
            });
    }, [services]);

    const handleCheckboxChange = useCallback((value: string, checked:boolean) => {
        setSelections(selections => ({...selections, [value]: checked}));
    }, []);

    const renderCat = useCallback((cat:PlaylistCategory) => {
        return <div className={'user-playlist__categories__category'} key={cat.id} data-tooltip={cat.name}>
            <Checkbox label={cat.name}
                      value={cat.id}
                      checked={selections[cat.id]}
                      onSelect={handleCheckboxChange}></Checkbox>
        </div>;
    }, [handleCheckboxChange, selections]);

    const handleSave = useCallback(() => {
        setLoading(true);
        const live = categories?.live?.filter(c => selections[c.id]);
        const vod = categories?.vod?.filter(c => selections[c.id]);
        const series = categories?.series?.filter(c => selections[c.id]);
        const bouquet: PlaylistCategories = {live, series, vod};
        services.userConfig().savePlaylistBouquet(bouquet).pipe(first(), finalize(() => setLoading(false))).subscribe({
            next: () => {
                enqueueSnackbar(translate('MESSAGES.SAVE.BOUQUET.SUCCESS'), {variant: 'success'})
            },
            error: () => {
                enqueueSnackbar(translate('MESSAGES.SAVE.BOUQUET.FAIL'), {variant: 'error'})
            }
        })
    }, [services, categories, selections, translate]);

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
        let filter_value = regexp ? filter : filter.toLowerCase();
        if (filter_value?.length) {
            const filtered = (categories as any)?.[activeTab]?.filter((cat: PlaylistCategory) => {
                if (regexp) {
                    // eslint-disable-next-line eqeqeq
                    return cat.name.trim().match(filter_value) != undefined;
                } else {
                    return (cat.name.trim().toLowerCase().indexOf(filter_value) > -1);
                }
            }) ?? [];
            setFilteredCategories(filteredCategories => ({...filteredCategories, [activeTab]: filtered}));
        } else {
            setFilteredCategories(filteredCategories => ({...filteredCategories, [activeTab]: (categories as any)?.[activeTab]}));
        }
    }, [activeTab, categories]);

    const handleShowSelected = useCallback((event: any) => {
        event.target.blur();
        setShowSelected(!showSelected);
    }, [showSelected]);

    return <>
        <LoadingIndicator loading={loading}></LoadingIndicator>
        <div className="user-playlist">
            <div className="user-playlist__toolbar">
                <label>{translate('TITLE.USER_BOUQUET_EDITOR')}</label>
                <button data-tooltip='LABEL.SAVE' onClick={handleSave}>{translate('LABEL.SAVE')}</button>
            </div>
            <TabSet tabs={tabs} active={activeTab} onTabChange={setActiveTab}></TabSet>
            {CATEGORY_TABS.map(tab => <div key={tab.key} className={'user-playlist__categories-panel' + (activeTab !== tab.key ? ' hidden' : '')}>
                <div className={'user-playlist__categories__toolbar'}>
                    <div className={'user-playlist__categories__toolbar-filter'}>
                        <PlaylistFilter onFilter={handleFilter}></PlaylistFilter>
                     </div>
                    <button className={showSelected ? 'button-active': ''} data-tooltip='LABEL.SHOW_SELECTED' onClick={handleShowSelected}>{getIconByName('Checked')}</button>
                    <button data-tooltip='LABEL.SELECT_ALL' onClick={handleSelectAll}>{getIconByName('SelectAll')}</button>
                    <button data-tooltip='LABEL.DESELECT_ALL' onClick={handleDeselectAll}>{getIconByName('DeselectAll')}</button>
                </div>
                <div className={'user-playlist__categories'}>
                    {getActiveCategories(tab.key)?.map(renderCat)}
                    </div>
                </div>)
            }
        </div>
    </>;
}
