import {Observable} from "rxjs";
import {ServerIpCheck, ServerStatus} from "../model/server-status";
import ServerStatusApiService, {DefaultServerStatusApiService} from "../api/server-status-api-service";

export default class ServerStatusService {

    constructor(private serverStatusApiService: ServerStatusApiService = new DefaultServerStatusApiService()) {
    }

    getServerStatus(): Observable<ServerStatus> {
        return this.serverStatusApiService.getServerStatus();
    }
    getIpCheck(): Observable<ServerIpCheck> {
        return this.serverStatusApiService.getServerIpCheck();
    }
}
