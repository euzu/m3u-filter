import ApiService, {DefaultApiService} from "./api-service";
import {PlaylistGroup} from "../model/playlist";
import {Observable, throwError} from "rxjs";
import {PlaylistRequest} from "../model/playlist-request";

const PLAYLIST_API_PATH = 'playlist';
const TARGET_UPDATE_API_PATH = 'playlist/update';

export default interface PlaylistApiService extends ApiService {
    getPlaylist(req: PlaylistRequest): Observable<PlaylistGroup[]>;

    updateTargets(targets: string[]): Observable<any>;
}

export class DefaultPlaylistApiService extends DefaultApiService implements PlaylistApiService {
    getPlaylist(req: PlaylistRequest): Observable<PlaylistGroup[]> {
        if (req.url || req.input_id != undefined) {
            return this.post<PlaylistGroup[]>(PLAYLIST_API_PATH, req);
        }
        return throwError(() => new Error('Invalid arguments'));
    }

    updateTargets(targets: string[]): Observable<any> {
        return this.post(TARGET_UPDATE_API_PATH, targets);
    }
}
