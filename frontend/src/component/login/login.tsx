import React, {useCallback, useRef, useState} from 'react';
import './login.scss';
import {useServices} from "../../provider/service-provider";
import {first} from "rxjs/operators";

const checkUserPwd = (username: string, password: string) => username.trim().length > 0 && password.trim().length > 8;

export default function Login(): JSX.Element {

    const usernameRef = useRef<HTMLInputElement>();
    const passwordRef = useRef<HTMLInputElement>();
    const services = useServices();
    const [authorized, setAuthorized] = useState(false);

    const handleLogin = useCallback(() => {
        const username = usernameRef.current.value;
        const password = passwordRef.current.value;
        services.auth().authenticate(username, password).pipe(first()).subscribe({
            next: (auth) => {
                setAuthorized(auth);
            },
            error: () => {
                setAuthorized(false);
            }
        });
    }, [services]);

    const handleKeyDown = useCallback((event: any) => {
        if (event.key === 'Enter') {
            if (checkUserPwd(usernameRef.current.value, passwordRef.current.value)) {
                handleLogin();
            }
        }
    }, [handleLogin]);


    return <div className={'login'}>
        <div className={'title'}>Login to m3u-filter</div>
        <form>
            <div className="login__form">
                <input ref={usernameRef} type="text" name="username" placeholder="username"/>
                <input ref={passwordRef} type="password" name="password" placeholder="password"
                       onKeyDown={handleKeyDown}/>
                <button type="button" className="btn" onClick={handleLogin}>Login</button>
                <span className={authorized ? '' : 'hidden'}>Failed to login</span>
            </div>
        </form>
    </div>

}