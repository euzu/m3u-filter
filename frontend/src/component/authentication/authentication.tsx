import React, {JSX, useEffect, useState} from 'react';
import App from "../../app/app";
import Login from "../login/login";
import {useServices} from "../../provider/service-provider";
import {first} from "rxjs/operators";
import {noop, tap} from "rxjs";
import {UserRole} from "../../service/auth-service";
import AppUser from "../../app-user/app-user";

export default function Authentication(): JSX.Element {

    const services = useServices();
    const [loading, setLoading] = useState<boolean>(true);
    const [authenticated, setAuthenticated] = useState<UserRole>(UserRole.NONE);

    useEffect(() => {
        const sub = services.auth().authChannel().subscribe({
            next: (auth) => {
                setLoading(false);
                setAuthenticated(auth);
            },
            error: () => setAuthenticated(UserRole.NONE),
        })

        const noAuthCheck = () => services.auth().authenticate('test', 'test')
            .pipe(tap(() => setLoading(false)), first()).subscribe(noop);

        services.auth().refresh().pipe(first()).subscribe({
            next: (authenticated) =>!authenticated && noAuthCheck(),
            error: () => noAuthCheck(),
        });

        return () => sub.unsubscribe();
    }, [services]);

    if (loading) {
        return <></>
    }

    if (authenticated === UserRole.ADMIN) {
        return <App/>;
    }

    if (authenticated === UserRole.USER) {
        return <AppUser/>;
    }

    return <Login/>
}