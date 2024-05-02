import React from 'react';
import { createRoot } from 'react-dom/client';
import './index.scss';
import {SnackbarProvider} from 'notistack';
import {ServiceProvider} from "./provider/service-provider";
import Authentication from "./component/authentication/authentication";

const container = document.getElementById('root');
const root = createRoot(container);
root.render(
    <SnackbarProvider maxSnack={3} autoHideDuration={1500} anchorOrigin={ ({vertical: 'top', horizontal: 'center'}) }>
        <ServiceProvider>
            <Authentication/>
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