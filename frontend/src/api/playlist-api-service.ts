import ApiService, {DefaultApiService} from "./api-service";
import {Observable, throwError} from "rxjs";
import {PlaylistRequest} from "../model/playlist-request";
import {PlaylistResponse} from "../model/playlist";

const PLAYLIST_API_PATH = 'playlist';
const TARGET_UPDATE_API_PATH = 'playlist/update';

export default interface PlaylistApiService extends ApiService {
    getPlaylist(req: PlaylistRequest): Observable<PlaylistResponse>;

    updateTargets(targets: string[]): Observable<any>;
}

export class DefaultPlaylistApiService extends DefaultApiService implements PlaylistApiService {
    getPlaylist(req: PlaylistRequest): Observable<PlaylistResponse> {
        // eslint-disable-next-line eqeqeq
        if (req != undefined) {
            return this.post<PlaylistResponse>(PLAYLIST_API_PATH, req);
        }
        return throwError(() => new Error('Invalid arguments'));
    }

    updateTargets(targets: string[]): Observable<any> {
        return this.post(TARGET_UPDATE_API_PATH, targets);
    }
}
