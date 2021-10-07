import ApiService, {DefaultApiService} from "./api-service";
import {Observable} from "rxjs";
import ServerConfig from "../model/server-config";

const SERVER_CONFIG_API_PATH = 'config';

export default interface ConfigApiService extends ApiService {
    getServerConfig(): Observable<ServerConfig>;
}

export class DefaultConfigApiService extends DefaultApiService implements ConfigApiService {
    getServerConfig(): Observable<ServerConfig> {
         return this.get<ServerConfig>(SERVER_CONFIG_API_PATH);
    }
}
