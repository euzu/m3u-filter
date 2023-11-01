import React from 'react';
import { createRoot } from 'react-dom/client';
import './index.scss';
import App from './app/app';
import {SnackbarProvider} from 'notistack';
import {ServiceProvider} from "./provider/service-provider";

const container = document.getElementById('root');
const root = createRoot(container);
root.render(    <SnackbarProvider maxSnack={3} autoHideDuration={1500}>
        <ServiceProvider>
            <App/>
        </ServiceProvider>
    </SnackbarProvider>
);

// ReactDOM.render(
//     <SnackbarProvider maxSnack={3} autoHideDuration={1500}>
//         <ServiceProvider>
//             <App/>
//         </ServiceProvider>
//     </SnackbarProvider>
//     , document.getElementById('root'));