import {Observable} from "rxjs";
import ApiService, {DefaultApiService} from "./api-service";
import {ServerStatus} from "../model/server-status";

const STATUS_PATH = "status";

export default interface ServerStatusApiService extends ApiService {
    getServerStatus(): Observable<ServerStatus>;
}

export class DefaultServerStatusApiService extends DefaultApiService implements ServerStatusApiService {

    getServerStatus(): Observable<ServerStatus> {
        return this.get<ServerStatus>(STATUS_PATH);
    }
}
