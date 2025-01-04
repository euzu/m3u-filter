import React, {JSX, useEffect, useState} from 'react';
import App from "../../app/app";
import Login from "../login/login";
import {useServices} from "../../provider/service-provider";
import {first} from "rxjs/operators";
import {noop, tap} from "rxjs";

export default function Authentication(): JSX.Element {

    const services = useServices();
    const [loading, setLoading] = useState<boolean>(true);
    const [authenticated, setAuthenticated] = useState<boolean>(false);

    useEffect(() => {
        const sub = services.auth().authChannel().subscribe({
            next: (auth) => setAuthenticated(auth),
            error: () => setAuthenticated(false),
        })

        const noAuthCheck = () => services.auth().authenticate('test', 'test').pipe(tap(() => setLoading(false)), first()).subscribe(noop);

        services.auth().refresh().pipe(first()).subscribe({
            next: (authenticated) =>!authenticated && noAuthCheck(),
            error: () => noAuthCheck(),
        });

        return () => sub.unsubscribe();
    }, [services]);

    if (loading) {
        return <></>
    }

    return authenticated ?  <App/> : <Login/>

}