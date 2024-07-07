import ApiService, {DefaultApiService} from "./api-service";
import {Observable} from "rxjs";

type TokenResponse = { token: string };

export default interface AuthApiService extends ApiService {
    authenticate(username: string, password: string): Observable<TokenResponse>;

    refresh(): Observable<TokenResponse>
}

export class DefaultAuthApiService extends DefaultApiService implements AuthApiService {
    private readonly authBaseUrl;
    constructor() {
        super();
        this.authBaseUrl = this.getBaseUrl() + '/auth';
    }
    authenticate(username: string, password: string): Observable<{ token: string }> {
        return this.post<TokenResponse>('/token', {username, password}, this.authBaseUrl);
    }

    refresh(): Observable<TokenResponse> {
        return this.post<TokenResponse>('/refresh', {}, this.authBaseUrl);
    }
}
