import React, {useCallback, useEffect, useRef, useState} from 'react';
import './user-playlist.scss';
import {useServices} from "../../provider/service-provider";
import {finalize, first, zip} from 'rxjs';
import {PlaylistCategories, PlaylistCategory} from "../../model/playlist-categories";
import LoadingIndicator from '../loading-indicator/loading-indicator';
import TabSet, {TabSetTab} from "../tab-set/tab-set";
import Checkbox from "../checkbox/checkbox";
import {enqueueSnackbar} from "notistack";

const CATEGORY_TABS = [
    {label: "Live", key: "live"},
    {label: "VOD", key: "vod"},
    {label: "Series", key: "series"}
];

/* eslint-disable @typescript-eslint/no-empty-interface */
interface UserPlaylistProps {

}

export default function UserPlaylist(props: UserPlaylistProps) {
    const services = useServices();
    const [loading, setLoading] = useState(false);
    const [categories, setCategories] = useState<PlaylistCategories>(undefined);
    const selections = useRef<Record<string, boolean>>(undefined);
    const [activeTab, setActiveTab] = useState<string>(CATEGORY_TABS[0].key);

    useEffect(() => {
        setLoading(true);
        zip(services.userConfig().getPlaylistBouquet().pipe(first()) ,
            services.userConfig().getPlaylistCategories().pipe(first())).pipe(finalize(() => setLoading(false)))
            .subscribe(([bouquet, categories]: [any, PlaylistCategories]) => {
                if (!bouquet && categories) {
                    const user_bouquet: any = {}
                    Object.values(categories).filter(Boolean).flat().forEach(c => user_bouquet[c.category_id] = true);
                    selections.current = user_bouquet;
                }
                setCategories(categories ?? undefined);
            });
    }, [services]);

    const handleCheckboxChange = useCallback((checked:boolean, value: string) => {
        selections.current[value] = checked;
    }, []);

    const renderCat = useCallback((cat:PlaylistCategory) => {
        return <div className={'user-playlist__categories__category'} key={cat.category_id}>
            <Checkbox label={cat.category_name}
                      value={cat.category_id}
                      checked={selections.current?.[cat.category_id]}
                      onSelect={handleCheckboxChange}></Checkbox>
        </div>;
    }, [handleCheckboxChange]);

    const handleTabChange = useCallback((target: string) => {
        setActiveTab(target);
    }, []);

    const handleSave = useCallback(() => {
        setLoading(true);
        const live = categories?.live?.filter(c => selections.current[c.category_id]);
        const vod = categories?.vod?.filter(c => selections.current[c.category_id]);
        const series = categories?.series?.filter(c => selections.current[c.category_id]);
        const bouquet: PlaylistCategories = {live, series, vod};
        services.userConfig().savePlaylistBouquet(bouquet).pipe(first(), finalize(() => setLoading(false))).subscribe({
            next: () => {
                enqueueSnackbar('Successfully save bouquet', {variant: 'success'})
            },
            error: () => {
                enqueueSnackbar('Failed to save bouquet', {variant: 'error'})
            }
        })
    }, [categories]);

    return <>
        <LoadingIndicator loading={loading}></LoadingIndicator>
        <div className="user-playlist">
            <div className="user-playlist__toolbar">
                <label>User boutique editor</label>
                <button title={'Save'} onClick={handleSave}>Save</button>
            </div>
            <TabSet tabs={CATEGORY_TABS} active={activeTab} onTabChange={handleTabChange}></TabSet>
            {CATEGORY_TABS.map(tab => <div className={'user-playlist__categories' + (activeTab !== tab.key ? ' hidden' : '')}>
                    {(categories as any)?.[tab.key]?.map(renderCat)}
                </div>)
            }
        </div>
    </>;
}
