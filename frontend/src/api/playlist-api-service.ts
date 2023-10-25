import ApiService, {DefaultApiService} from "./api-service";
import {PlaylistGroup} from "../model/playlist";
import {Observable, throwError} from "rxjs";
import {PlaylistRequest} from "../model/playlist-request";

const PLAYLIST_API_PATH = 'playlist';

export default interface PlaylistApiService extends ApiService {
    getPlaylist(req: PlaylistRequest): Observable<PlaylistGroup[]>;
}

export class DefaultPlaylistApiService extends DefaultApiService implements PlaylistApiService {
    getPlaylist(req: PlaylistRequest): Observable<PlaylistGroup[]> {
        if (req.url || req.input_id != undefined) {
            return this.post<PlaylistGroup[]>(PLAYLIST_API_PATH, req);
        }
        return throwError(() => new Error('Invalid arguments'));
    }
}
