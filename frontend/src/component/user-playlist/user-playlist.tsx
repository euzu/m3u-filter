import React, {useCallback, useEffect, useRef, useState} from 'react';
import './user-playlist.scss';
import {useServices} from "../../provider/service-provider";
import {finalize, first, zip} from 'rxjs';
import {UserPlaylistCategories, UserPlaylistTargetCategories} from "../../model/playlist";
import LoadingIndicator from '../loading-indicator/loading-indicator';
import useTranslator from "../../hook/use-translator";
import UserTargetPlaylist, {BouquetSelection} from "../user-target-playlist/user-target-playlist";
import {useSnackbar} from "notistack";

const TARGET_TABS = [
    {label: "LABEL.XC", key: "xtream"},
    {label: "LABEL.M3U", key: "m3u"},
];

function isEmpty(value: any): boolean {
    if (value == null) return true;
    if (typeof value === "string") return value.trim() === "";
    if (Array.isArray(value)) return value.length === 0;
    if (typeof value === "object") return Object.keys(value).length === 0;
    return false;
}

/* eslint-disable @typescript-eslint/no-empty-interface */
interface UserPlaylistProps {

}

export default function UserPlaylist(props: UserPlaylistProps) {
    const services = useServices();
    const translate = useTranslator();
    const [loading, setLoading] = useState(false);
    const [categories, setCategories] = useState<UserPlaylistCategories>(undefined);
    const [bouquets, setBouquets] = useState<UserPlaylistCategories>(undefined);
    const [activeTab, setActiveTab] = useState<string>(TARGET_TABS[0].key);
    const selectionRef = useRef<{ xtream: BouquetSelection, m3u: BouquetSelection }>({} as any);
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();

    useEffect(() => {
        setLoading(true);
        zip(services.userConfig().getPlaylistBouquet().pipe(first()),
            services.userConfig().getPlaylistCategories().pipe(first())).pipe(finalize(() => setLoading(false)))
            .subscribe(([bouquet, categories]: [UserPlaylistCategories, UserPlaylistCategories]) => {
                if (isEmpty(bouquet)) {
                    bouquet = undefined;
                }
                if (isEmpty(categories)) {
                    categories = undefined;
                }
                setCategories(categories);
                setBouquets(bouquet);

                const mapToSelection = (userBouquet: string[]) => {
                    if (userBouquet) {
                        return userBouquet.reduce((acc: any, e: string) => {
                            acc[e] = true;
                            return acc;
                        }, {})
                    }
                    return undefined;
                }
                const mapTargetToSelection = (targetBouquet: UserPlaylistTargetCategories) => {
                    return {
                        live: mapToSelection(targetBouquet?.live),
                        vod: mapToSelection(targetBouquet?.vod),
                        series: mapToSelection(targetBouquet?.series),
                    }
                }

                selectionRef.current = {
                    xtream: mapTargetToSelection(bouquet?.xtream),
                    m3u: mapTargetToSelection(bouquet?.m3u),
                };
            });
    }, [services]);

    const handleActiveTabChange = useCallback((event: any) => {
        const tab = event.target.dataset.tab;
        setActiveTab(tab);
    }, []);

    const handleSave = useCallback(() => {
        setLoading(true);

        const toClusterCategories = (clusterBouquet: any, clusterCategories: any): string[] => {
            const result = clusterBouquet ? Object.keys(clusterBouquet).filter(key => clusterBouquet[key]) : undefined;
            if (result?.length === clusterCategories?.length) {
                return undefined;
            }
            return result;
        }
        const toTargetCategories = (bs: BouquetSelection, targetCategories: UserPlaylistTargetCategories): UserPlaylistTargetCategories => ({
            live: toClusterCategories(bs?.live, targetCategories?.live),
            vod: toClusterCategories(bs?.vod, targetCategories?.vod),
            series: toClusterCategories(bs?.series, targetCategories?.series)
        });

        const bouquet: UserPlaylistCategories = {
            xtream: toTargetCategories(selectionRef.current.xtream, categories.xtream),
            m3u: toTargetCategories(selectionRef.current.m3u, categories.m3u)
        }

        services.userConfig().savePlaylistBouquet(bouquet).pipe(first(), finalize(() => setLoading(false))).subscribe({
            next: () => {
                enqueueSnackbar(translate('MESSAGES.SAVE.BOUQUET.SUCCESS'), {variant: 'success'})
            },
            error: () => {
                enqueueSnackbar(translate('MESSAGES.SAVE.BOUQUET.FAIL'), {variant: 'error'})
            }
        })
    }, [services, translate, enqueueSnackbar, categories?.m3u, categories?.xtream]);

    const handleSelectionChange = (selections: BouquetSelection) => {
        (selectionRef.current as any)[activeTab] = selections;
    }

    return <>
        <LoadingIndicator loading={loading}></LoadingIndicator>
        <div className="user-playlist">
            <div className="user-playlist__toolbar">
                <label>{translate('TITLE.USER_BOUQUET_EDITOR')}</label>
                <button data-tooltip='LABEL.SAVE' onClick={handleSave}>{translate('LABEL.SAVE')}</button>
            </div>
            <div className="user-playlist__content">
                <div className="user-playlist__content-toolbar">
                    {TARGET_TABS.map((t) =>
                        <button key={t.key} className={activeTab === t.key ? 'button-active' : ''} data-tab={t.key}
                                onClick={handleActiveTabChange}>{translate(t.label)}</button>)}
                </div>
                <div className="user-playlist__content-panels">
                    {TARGET_TABS.map((t) =>
                        <UserTargetPlaylist onSelectionChange={handleSelectionChange}
                                            key={t.key} visible={activeTab === t.key}
                                            bouquet={(bouquets as any)?.[t.key]}
                                            categories={(categories as any)?.[t.key]}></UserTargetPlaylist>)}
                </div>
            </div>
        </div>
    </>;
}
