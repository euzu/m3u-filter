import ApiService, {DefaultApiService} from "./api-service";
import {Observable} from "rxjs";
import ServiceContext from "../service/service-context";

type TokenResponse = { token: string };

export default interface AuthApiService extends ApiService {
    authenticate(username: string, password: string): Observable<TokenResponse>;

    refresh(): Observable<TokenResponse>
}

export class DefaultAuthApiService extends DefaultApiService implements AuthApiService {
    constructor() {
        super();
    }

    private getAuthUrl() : string {
        let authUrl =  ServiceContext.config().getUiConfig().api.authUrl;
        if (authUrl.endsWith("/")) {
            authUrl = authUrl.slice(0, -1);
        }
        console.log('authUrl', authUrl);
        return authUrl;
    }

    authenticate(username: string, password: string): Observable<{ token: string }> {
        const authBaseUrl = this.getAuthUrl();
        return this.post<TokenResponse>('/token', {username, password}, authBaseUrl);
    }

    refresh(): Observable<TokenResponse> {
        const authBaseUrl = this.getAuthUrl();
        return this.post<TokenResponse>('/refresh', {}, authBaseUrl);
    }
}
