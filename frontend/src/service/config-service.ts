import ConfigApiService, {DefaultConfigApiService} from "../api/config-api-service";
import {Observable} from "rxjs";
import ServerConfig, {ApiProxyServerInfo, ServerMainConfig, TargetUser} from "../model/server-config";
import {DefaultUiConfig, UiConfig} from "../model/ui-config";

export default class ConfigService {

    private uiConfig: UiConfig = DefaultUiConfig;

    constructor(private configApiService: ConfigApiService = new DefaultConfigApiService()) {
    }

    setUiConfig(uiConfig: UiConfig): void {
        if (uiConfig) {
            this.uiConfig = {...DefaultUiConfig, ...uiConfig};
        }
    }

    getUiConfig(): UiConfig {
        return this.uiConfig;
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
