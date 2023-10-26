import ConfigApiService, {DefaultConfigApiService} from "../api/config-api-service";
import {Observable} from "rxjs";
import ServerConfig, {ServerInfo, TargetUser} from "../model/server-config";

export default class ConfigService {
    constructor(private configApiService: ConfigApiService = new DefaultConfigApiService()) {
    }

    getServerConfig(): Observable<ServerConfig> {
        return this.configApiService.getServerConfig();
    }

    saveTargetUser(targetUser: TargetUser[]) {
        return this.configApiService.saveTargetUser(targetUser);
    }

    saveServerInfo(serverInfo: ServerInfo) {
        return this.configApiService.saveServerInfo(serverInfo);
    }
}
