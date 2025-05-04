import {Observable} from "rxjs";
import ApiService, {DefaultApiService} from "./api-service";
import {ServerIpCheck, ServerStatus} from "../model/server-status";

const STATUS_PATH = "status";
const IPCHECK_PATH = "ipinfo";

export default interface ServerStatusApiService extends ApiService {
    getServerStatus(): Observable<ServerStatus>;

    getServerIpCheck(): Observable<ServerIpCheck>;
}

export class DefaultServerStatusApiService extends DefaultApiService implements ServerStatusApiService {

    getServerStatus(): Observable<ServerStatus> {
        return this.get<ServerStatus>(STATUS_PATH);
    }

    getServerIpCheck(): Observable<ServerIpCheck> {
        return this.get<ServerIpCheck>(IPCHECK_PATH);
    }
}
