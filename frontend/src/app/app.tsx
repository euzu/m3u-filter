import React, {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import './app.scss';
import {useSnackbar} from 'notistack';
import {useServices} from "../provider/service-provider";
import {first} from "rxjs/operators";
import {noop} from "rxjs";
import ServerConfig from "../model/server-config";
import {getIconByName} from "../icons/icons";
import Preferences from "../component/preferences/preferences";
import useTranslator, {LANGUAGE} from "../hook/use-translator";
import i18next from "i18next";
import PlaylistBrowser from '../component/playlist-browser/playlist-browser';
import SetupWizard from "../component/setup-wizard/setup-wizard";

/* eslint-disable @typescript-eslint/no-empty-interface */
interface AppProps {

}

export default function App(props: AppProps) {
    const services = useServices();
    const [serverConfig, setServerConfig] = useState<ServerConfig>(undefined);
    const serverConfigLoaded = useRef(false);
    const [preferencesVisible, setPreferencesVisible] = useState<boolean>(true);
    const appTitle = useMemo(() => services.config().getUiConfig().app_title ?? 'm3u-filter', [services]);
    const translate = useTranslator();
    const appLogo = useMemo(() => {
        let logo =  services.config().getUiConfig().app_logo;
        if (logo) {
            return <img src={logo} alt="logo" />;
        } else {
            return getIconByName('Logo');
        }
    }, [services]);
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();

    useEffect(() => {
        if (!serverConfigLoaded.current) {
            serverConfigLoaded.current = true;
            services.config().getServerConfig().pipe(first()).subscribe({
                next: (cfg: ServerConfig) => {
                    setServerConfig(cfg);
                },
                error: (err) => {
                    enqueueSnackbar(translate("MESSAGES.DOWNLOAD.SERVER_CONFIG.FAIL"), {variant: 'error'});
                },
                complete: noop,
            });
        }
        return noop
    }, [enqueueSnackbar, services, translate]);

    const handlePreferences = useCallback(() => {
       setPreferencesVisible((value:boolean) => !value);
    }, []);

    const handleLogout = useCallback(() => {
        services.auth().logout();
    }, [services]);

    const handleLanguage = useCallback((event: any) => {
        const language = event.target.value;
        LANGUAGE.next(language);
    }, []);

    return (
        <div className="app">
            <div className={'app-header'}>
                <div className={'app-header__caption'}><span className={'app-header__logo'}>{appLogo}</span>{appTitle}</div>
                <div className={'app-header__toolbar'}><select onChange={handleLanguage} defaultValue={i18next.language}>{services.config().getUiConfig().languages.map(l => <option key={l} value={l}>{l}</option>)}</select></div>
                <div className={'app-header__toolbar'}><button data-tooltip={preferencesVisible ? 'LABEL.PLAYLIST_BROWSER' : 'LABEL.CONFIGURATION'} onClick={handlePreferences}>{getIconByName(preferencesVisible ? 'Live' : 'Config')}</button></div>
                <div className={'app-header__toolbar'}><button data-tooltip='LABEL.LOGOUT' onClick={handleLogout}>{getIconByName('Logout')}</button></div>
            </div>
            {/*<div className={'app-main'}>*/}
            {/*    <div className={'app-content'}>*/}
            {/*        <SetupWizard />*/}
            {/*    </div>*/}
            {/*</div>*/}

            <div className={'app-main' + (preferencesVisible ? '' : '  hidden')}>
                <div className={'app-content'}>
                    <Preferences config={serverConfig} />
                </div>
            </div>
            <div className={'app-main' + (preferencesVisible ? ' hidden' : '')}>
                <PlaylistBrowser config={serverConfig} />
            </div>
        </div>
    );
}
