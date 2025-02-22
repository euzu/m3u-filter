import {
    catchError,
    concatWith, EMPTY,
    interval,
    map,
    Observable,
    of,
    ReplaySubject,
    takeWhile,
    tap,
    throwError
} from "rxjs";
import AuthApiService, {DefaultAuthApiService} from "../api/auth-api-service";
import {first} from "rxjs/operators";
import {jwtDecode} from "jwt-decode";

export enum UserRole {
    NONE = 0,
    ADMIN = 1,
    USER= 2,
}

const AUTH_TOKEN_KEY = "auth-token";
const NO_AUTHORIZATION_TOKEN = "authorized";

const REFRESH_INTERVAL = 1000 * 60 * 15; // 15 mins

export default class AuthService {

    private token: string;
    private subject = new ReplaySubject<UserRole>(UserRole.NONE);

    constructor(private authApiService: AuthApiService = new DefaultAuthApiService()) {
        this.token = localStorage.getItem(AUTH_TOKEN_KEY);
        interval(REFRESH_INTERVAL).pipe(
            takeWhile(() => this.token !== NO_AUTHORIZATION_TOKEN),
            concatWith(EMPTY)).subscribe(() => this.refresh().pipe(first()).subscribe({
            next: (success: boolean) => {
                if (success) {
                    this.subject.next(this.getRole());
                } else {
                    this.clearToken();
                }
            },
            error: () => this.clearToken(),
        }));
    }

    private clearToken() {
        localStorage.removeItem(AUTH_TOKEN_KEY);
        this.subject.next(UserRole.NONE);
    }

    private getRole(): UserRole {
        if (this.token) {
            try {
                if (this.token === NO_AUTHORIZATION_TOKEN) {
                    return UserRole.ADMIN;
                }
                const claims: any = jwtDecode(this.token);
                if (claims?.roles?.includes("ADMIN")) {
                    return UserRole.ADMIN;
                }
                if (claims?.roles?.includes("USER")) {
                    return UserRole.USER;
                }
            } catch(err) {
                this.clearToken();
            }
        }
        return UserRole.NONE;
    }

    authChannel(): Observable<UserRole> {
        return this.subject;
    }

    refresh(): Observable<boolean> {
        if (this.token) {
            return this.authApiService.refresh().pipe(map(auth => {
                this.token = auth.token;
                localStorage.setItem(AUTH_TOKEN_KEY, auth.token);
                // eslint-disable-next-line eqeqeq
                return auth.token != undefined
            }), tap(data => {
                this.subject.next(this.getRole());
            }), catchError((error: any) => {
                this.subject.next(UserRole.NONE);
                return throwError(() => error)
            }));
        }
        return of(false);
    }

    authenticate(username: string, password: string): Observable<boolean> {
        return this.authApiService.authenticate(username, password).pipe(map(auth => {
            this.token = auth.token;
            localStorage.setItem(AUTH_TOKEN_KEY, auth.token);
            // eslint-disable-next-line eqeqeq
            return auth.token != undefined
        }), tap(data => {
            this.subject.next(this.getRole());
        }), catchError(() => {
            this.clearToken();
            return of(false);
        }));
    }

    isAuthenticated(): boolean {
        // eslint-disable-next-line eqeqeq
        return this.token != undefined;
    }

    getToken(): string {
        return this.token;
    }

    logout() {
        this.token = null;
        localStorage.removeItem(AUTH_TOKEN_KEY);
        this.subject.next(UserRole.NONE);
    }
}