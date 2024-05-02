import {catchError, map, Observable, ReplaySubject, tap, throwError} from "rxjs";
import AuthApiService, {DefaultAuthApiService} from "../api/auth-api-service";

export default class AuthService {

    private token: string;
    private subject = new ReplaySubject<boolean>(1);

    constructor(private authApiService: AuthApiService = new DefaultAuthApiService()) {
        this.subject.next(false);
    }

    authChannel(): Observable<boolean> {
        return this.subject;
    }

    authenticate(username: string, password: string): Observable<boolean> {
        return this.authApiService.authenticate(username, password).pipe(map(auth => {
            this.token = auth.token;
            return auth.token != null
        }), tap(data => {
            this.subject.next(data);
        }), catchError((error:any) => {
            this.subject.next(false);
            return throwError(() => error)
        }));
    }

    isAuthenticated(): boolean {
        return this.token != null;
    }

    getToken(): string  {
        return this.token;
    }
}