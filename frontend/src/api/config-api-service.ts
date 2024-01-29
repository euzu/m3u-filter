import ApiService, {DefaultApiService} from "./api-service";
import {Observable} from "rxjs";
import ServerConfig, {ApiProxyServerInfo, ServerMainConfig, TargetUser} from "../model/server-config";

const SERVER_CONFIG_API_PATH = 'config';
const SERVER_CONFIG_MAIN_API_PATH = 'config/main';
const SERVER_CONFIG_TARGET_USER_API_PATH = 'config/user';
const SERVER_CONFIG_SERVER_INFO_API_PATH = 'config/apiproxy';

export default interface ConfigApiService extends ApiService {
    getServerConfig(): Observable<ServerConfig>;

    saveMainConfig(config: ServerMainConfig): Observable<any>;

    saveTargetUser(targetUser: TargetUser[]): Observable<any>;

    saveApiProxyConfig(serverInfo: ApiProxyServerInfo[]): Observable<any>;
}

export class DefaultConfigApiService extends DefaultApiService implements ConfigApiService {
    getServerConfig(): Observable<ServerConfig> {
        return this.get<ServerConfig>(SERVER_CONFIG_API_PATH);
    }

    saveMainConfig(config: ServerMainConfig): Observable<any> {
        return this.post<ServerConfig>(SERVER_CONFIG_MAIN_API_PATH, config);
    }

    saveTargetUser(targetUser: TargetUser[]): Observable<any> {
        return this.post<ServerConfig>(SERVER_CONFIG_TARGET_USER_API_PATH, targetUser);
    }

    saveApiProxyConfig(serverInfo: ApiProxyServerInfo[]): Observable<any> {
        return this.post<ServerConfig>(SERVER_CONFIG_SERVER_INFO_API_PATH, serverInfo);
    }

}
