import ConfigApiService, {DefaultConfigApiService} from "../api/config-api-service";
import {Observable} from "rxjs";
import ServerConfig, {ApiProxyServerInfo, ServerMainConfig, TargetUser} from "../model/server-config";

export default class ConfigService {
    constructor(private configApiService: ConfigApiService = new DefaultConfigApiService()) {
    }

    getServerConfig(): Observable<ServerConfig> {
        return this.configApiService.getServerConfig();
    }

    saveMainConfig(config: ServerMainConfig) {
        return this.configApiService.saveMainConfig(config);
    }

    saveTargetUser(targetUser: TargetUser[]) {
        return this.configApiService.saveTargetUser(targetUser);
    }

    saveApiProxyConfig(serverInfo: ApiProxyServerInfo[]) {
        return this.configApiService.saveApiProxyConfig(serverInfo);
    }
}
