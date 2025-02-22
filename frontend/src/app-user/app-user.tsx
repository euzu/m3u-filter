import React, {useCallback, useMemo} from 'react';
import './app-user.scss';
import {getIconByName} from "../icons/icons";
import {useServices} from "../provider/service-provider";
import UserPlaylist from "../component/user-playlist/user-playlist";
import useTranslator, {LANGUAGE} from "../hook/use-translator";
import i18next from "i18next";

/* eslint-disable @typescript-eslint/no-empty-interface */
interface AppUserProps {

}

export default function AppUser(props: AppUserProps) {
    const services = useServices();
    const translate = useTranslator();
    const appTitle = useMemo(() => services.config().getUiConfig().app_title ?? 'm3u-filter', [services]);
    const appLogo = useMemo(() => {
       let logo =  services.config().getUiConfig().app_logo;
       if (logo) {
           return <img src={logo} alt="logo" />;
       } else {
           return getIconByName('Logo');
       }
    }, [services]);
    const handleLogout = useCallback(() => {
        services.auth().logout();
    }, [services]);

    const handleLanguage = useCallback((event: any) => {
        const language = event.target.value;
        LANGUAGE.next(language);
    }, []);


    return (
        <div className="user-app">
            <div className={'user-app__header'}>
                <div className={'user-app__header__caption'}><span className={'user-app__header__logo'}>{appLogo}</span>{appTitle}</div>
                <div className={'app-header__toolbar'}><select onChange={handleLanguage}>{services.config().getUiConfig().languages.map(l => <option key={l} value={l} selected={l === i18next.language}>{l}</option>)}</select></div>
                <div className={'user-app__header__toolbar'}><button title={translate('LABEL.LOGOUT')} onClick={handleLogout}>{getIconByName('Logout')}</button></div>
            </div>
            <div className={'user-app__main'}>
                <div className={'user-app__content'}>
                    <UserPlaylist></UserPlaylist>
                </div>
            </div>
        </div>
    );
}
