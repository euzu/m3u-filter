import React from 'react';
import ReactDOM from 'react-dom';
import './index.scss';
import App from './app/app';
import {SnackbarProvider} from 'notistack';
import {ServiceProvider} from "./provider/service-provider";

ReactDOM.render(
    <SnackbarProvider maxSnack={3}>
        <ServiceProvider>
            <App/>
        </ServiceProvider>
    </SnackbarProvider>
    , document.getElementById('root'));