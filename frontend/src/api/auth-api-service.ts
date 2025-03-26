import ApiService, {DefaultApiService} from "./api-service";
import {Observable} from "rxjs";

type TokenResponse = { token: string };

export default interface AuthApiService extends ApiService {
    authenticate(username: string, password: string): Observable<TokenResponse>;

    refresh(): Observable<TokenResponse>
}

export class DefaultAuthApiService extends DefaultApiService implements AuthApiService {
    constructor() {
        super();
    }
    authenticate(username: string, password: string): Observable<{ token: string }> {
        const authBaseUrl = this.getBaseUrl() + '/auth';
        return this.post<TokenResponse>('/token', {username, password}, authBaseUrl);
    }

    refresh(): Observable<TokenResponse> {
        const authBaseUrl = this.getBaseUrl() + '/auth';
        return this.post<TokenResponse>('/refresh', {}, authBaseUrl);
    }
}
