import React, {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import './user-app.scss';
import {getIconByName} from "../icons/icons";
import {useServices} from "../provider/service-provider";

/* eslint-disable @typescript-eslint/no-empty-interface */
interface UserAppProps {

}

export default function UserApp(props: UserAppProps) {
    const services = useServices();
    const handleLogout = useCallback(() => {
        services.auth().logout();
    }, []);

    return (
        <div className="user-app">
            <div className={'user-app__header'}>
                <div className={'user-app__header__caption'}><span className={'user-app__header__logo'}>{getIconByName('Logo')}</span>{services.config().getUiConfig().app_title}</div>
                <div className={'user-app__header__toolbar'}><button title="Logout" onClick={handleLogout}>{getIconByName('Logout')}</button></div>
            </div>
            <div className={'user-app__main'}>
                <div className={'user-app__content'}>
                    Hello
                </div>
            </div>
        </div>
    );
}
