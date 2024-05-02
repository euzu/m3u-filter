import ApiService, {DefaultApiService} from "./api-service";
import {Observable} from "rxjs";

const AUTH_API_PATH = window.location + 'auth';

type TokenResponse = { token: string };

export default interface AuthApiService extends ApiService {
    authenticate(username: string, password: string): Observable<TokenResponse>;

    refresh(): Observable<TokenResponse>
}

export class DefaultAuthApiService extends DefaultApiService implements AuthApiService {
    authenticate(username: string, password: string): Observable<{ token: string }> {
        return this.post<TokenResponse>('/token', {username, password}, AUTH_API_PATH);
    }

    refresh(): Observable<TokenResponse> {
        return this.post<TokenResponse>('/refresh', {}, AUTH_API_PATH);
    }
}
