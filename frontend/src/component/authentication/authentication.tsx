import React, {useEffect, useState} from 'react';
import App from "../../app/app";
import Login from "../login/login";
import {useServices} from "../../provider/service-provider";

export default function Authentication(): JSX.Element {

    const services = useServices();
    const [authenticated, setAuthenticated] = useState<boolean>(false);

    useEffect(() => {
        const sub = services.auth().authChannel().subscribe({
            next: (auth) => setAuthenticated(auth),
            error: () => setAuthenticated(false),
        })
        return () => sub.unsubscribe();
    }, [services]);

    return <>{authenticated ?  <App/> : <Login/>}</>

}