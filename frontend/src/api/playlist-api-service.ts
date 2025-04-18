import ApiService, {DefaultApiService} from "./api-service";
import {Observable, throwError} from "rxjs";
import {PlaylistRequest} from "../model/playlist-request";
import {PlaylistCategories, PlaylistItem} from "../model/playlist";

const PLAYLIST_API_PATH = 'playlist';
const TARGET_UPDATE_API_PATH = 'playlist/update';
const REVERSE_URL_API_PATH = 'playlist/reverse';
const WEBPLAYER_URL_API_PATH = 'playlist/webplayer';

export default interface PlaylistApiService extends ApiService {
    getPlaylist(req: PlaylistRequest): Observable<PlaylistCategories>;

    updateTargets(targets: string[]): Observable<any>;

    getWebPlayerUrl(item: PlaylistItem, req: PlaylistRequest): Observable<string>;
}

export class DefaultPlaylistApiService extends DefaultApiService implements PlaylistApiService {
    getPlaylist(req: PlaylistRequest): Observable<PlaylistCategories> {
        // eslint-disable-next-line eqeqeq
        if (req != undefined) {
            return this.post<PlaylistCategories>(PLAYLIST_API_PATH, req);
        }
        return throwError(() => new Error('Invalid arguments'));
    }

    updateTargets(targets: string[]): Observable<any> {
        return this.post(TARGET_UPDATE_API_PATH, targets);
    }

    getWebPlayerUrl(item: PlaylistItem, req: PlaylistRequest): Observable<string> {
        return this.post(WEBPLAYER_URL_API_PATH + '/' + req.sourceId, item);
    }

}
