import React, {useEffect, useState} from 'react';
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

        services.auth().authenticate('test', 'test').pipe(tap(() => setLoading(false)), first()).subscribe(noop);

        return () => sub.unsubscribe();
    }, [services]);

    if (loading) {
        return <></>
    }

    return <>{authenticated ?  <App/> : <Login/>}</>

}