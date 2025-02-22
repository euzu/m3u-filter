import React from 'react';
import {createRoot} from 'react-dom/client';
import './index.scss';
import {SnackbarProvider} from 'notistack';
import {ServiceProvider} from "./provider/service-provider";
import Authentication from "./component/authentication/authentication";
import Fetcher from "./utils/fetcher";
import ServiceContext from "./service/service-context";
import {UiConfig} from './model/ui-config';
import i18n_init from "./utils/i18n";

import {catchError, map, switchMap} from 'rxjs/operators';
import {EMPTY} from 'rxjs';

const initUI = () => {
    const container = document.getElementById('root');
    const root = createRoot(container);
    root.render(
        <SnackbarProvider maxSnack={3} autoHideDuration={1500} anchorOrigin={({vertical: 'top', horizontal: 'center'})}>
            <ServiceProvider>
                <Authentication/>
            </ServiceProvider>
        </SnackbarProvider>
    );
}

Fetcher.fetchJson("config.json").pipe(
    switchMap((config: UiConfig) => {
            ServiceContext.config().setUiConfig(config);
            return i18n_init(ServiceContext.config().getUiConfig().languages).pipe(
                map(() => {
                    initUI();
                })
            )
        }
    ),
    catchError((error: Error) => {
        initUI();
        return EMPTY;
    })
).subscribe();

// Fetcher.fetchJson("config.json").subscribe({
//     next: (config: UiConfig) => {
//         i18n_init(config.languages).subscribe(
//         ServiceContext.config().setUiConfig(config);
//         initUI();
//     },
//     error: (error: Error) => {initUI()}
// });

