import React, {JSX, useCallback, useRef, useState} from 'react';
import './login.scss';
import {useServices} from "../../provider/service-provider";
import {first} from "rxjs/operators";

const checkUserPwd = (username: string, password: string) => username.trim().length > 0 && password.trim().length > 0;

export default function Login(): JSX.Element {

    const usernameRef = useRef<HTMLInputElement>(undefined);
    const passwordRef = useRef<HTMLInputElement>(undefined);
    const services = useServices();
    const [authorized, setAuthorized] = useState(true);

    const handleLogin = useCallback(() => {
        const username = usernameRef.current.value;
        const password = passwordRef.current.value;
        services.auth().authenticate(username, password).pipe(first()).subscribe({
            next: (auth) => setAuthorized(auth),
            error: () => setAuthorized(false)
        });
    }, [services]);

    const handleKeyDown = useCallback((event: any) => {
        if (event.code === 'Enter') {
            if (checkUserPwd(usernameRef.current.value, passwordRef.current.value)) {
                handleLogin();
            }
        }
    }, [handleLogin]);


    return <div className={'login-view'}>
        <div className={'login-view__title'}>Login to {services.config().getUiConfig().app_title ?? 'm3u-filter'}</div>
        <form>
            <div className="login-view__form">
                <input ref={usernameRef} type="text" name="username" placeholder="username"/>
                <input ref={passwordRef} type="password" name="password" placeholder="password" onKeyDown={handleKeyDown}/>
                <button type="button" onClick={handleLogin}>Login</button>
                <span className={authorized ? 'hidden' : 'error-text'}>Failed to login</span>
            </div>
        </form>
    </div>

}