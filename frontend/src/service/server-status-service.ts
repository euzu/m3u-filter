import {Observable} from "rxjs";
import {ServerStatus} from "../model/server-status";
import ServerStatusApiService, {DefaultServerStatusApiService} from "../api/server-status-api-service";

export default class ServerStatusService {

    constructor(private serverStatusApiService: ServerStatusApiService = new DefaultServerStatusApiService()) {
    }

    getServerStatus(): Observable<ServerStatus> {
        return this.serverStatusApiService.getServerStatus();
    }
}
