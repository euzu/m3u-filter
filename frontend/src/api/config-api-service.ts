import ApiService, {DefaultApiService} from "./api-service";
import {Observable} from "rxjs";
import ServerConfig, {TargetUser} from "../model/server-config";

const SERVER_CONFIG_API_PATH = 'config';
const SERVER_CONFIG_TARGET_USER_API_PATH = 'config/user';

export default interface ConfigApiService extends ApiService {
    getServerConfig(): Observable<ServerConfig>;

    saveTargetUser(targetUser: TargetUser[]): Observable<any>;
}

export class DefaultConfigApiService extends DefaultApiService implements ConfigApiService {
    getServerConfig(): Observable<ServerConfig> {
        return this.get<ServerConfig>(SERVER_CONFIG_API_PATH);
    }

    saveTargetUser(targetUser: TargetUser[]): Observable<any> {
        return this.post<ServerConfig>(SERVER_CONFIG_TARGET_USER_API_PATH, targetUser);
    }

}
