import {
    catchError,
    interval,
    map,
    noop,
    Observable,
    of,
    ReplaySubject,
    takeWhile,
    tap,
    throwError
} from "rxjs";
import AuthApiService, {DefaultAuthApiService} from "../api/auth-api-service";
import {first} from "rxjs/operators";

const REFRESH_INTERVAL = 1000 * 60 * 15; // 15 mins

export default class AuthService {

    private token: string;
    private subject = new ReplaySubject<boolean>(1);

    constructor(private authApiService: AuthApiService = new DefaultAuthApiService()) {
        this.subject.next(false);
        interval(REFRESH_INTERVAL).pipe(takeWhile(ev => this.token !== 'authorized')).subscribe(() => this.refresh().pipe(first()).subscribe(noop));
    }

    authChannel(): Observable<boolean> {
        return this.subject;
    }

    refresh(): Observable<boolean> {
        if (this.token) {
            return this.authApiService.refresh().pipe(map(auth => {
                this.token = auth.token;
                return auth.token != null
            }), tap(data => {
                this.subject.next(data);
            }), catchError((error: any) => {
                this.subject.next(false);
                return throwError(() => error)
            }));
        }
        return of(false);
    }

    authenticate(username: string, password: string): Observable<boolean> {
        return this.authApiService.authenticate(username, password).pipe(map(auth => {
            this.token = auth.token;
            return auth.token != null
        }), tap(data => {
            this.subject.next(data);
        }), catchError(() => of(false)));
    }

    isAuthenticated(): boolean {
        return this.token != null;
    }

    getToken(): string {
        return this.token;
    }
}