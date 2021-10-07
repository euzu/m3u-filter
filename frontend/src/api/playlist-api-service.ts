import ApiService, {DefaultApiService} from "./api-service";
import {PlaylistGroup} from "../model/playlist";
import {Observable} from "rxjs";

const PLAYLIST_API_PATH = 'playlist';

export default interface PlaylistApiService extends ApiService {
    getPlaylist(url: string): Observable<PlaylistGroup[]>;
}

export class DefaultPlaylistApiService extends DefaultApiService implements PlaylistApiService {
    getPlaylist(url: string): Observable<PlaylistGroup[]> {
        return this.post<PlaylistGroup[]>(PLAYLIST_API_PATH, {url});
    }

}
