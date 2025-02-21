import {Observable, of} from "rxjs";
import ApiService, {DefaultApiService} from "./api-service";
import {PlaylistCategories} from "../model/playlist-categories";

const PLAYLIST_CATEGORIES_PATH = "user/playlist/categories";
const PLAYLIST_BOUQUET_PATH = "user/playlist/bouquet";

export default interface UserConfigApiService extends ApiService {
    getPlaylistBouquet(): Observable<PlaylistCategories>;
    getPlaylistCategories(): Observable<PlaylistCategories>;
    savePlaylistBouquet(bouquet: PlaylistCategories): Observable<void>;
}

export class DefaultUserConfigApiService extends DefaultApiService implements UserConfigApiService {

    getPlaylistBouquet(): Observable<PlaylistCategories> {
        return this.get<PlaylistCategories>(PLAYLIST_BOUQUET_PATH);
    }
    getPlaylistCategories(): Observable<PlaylistCategories> {
        return this.get<PlaylistCategories>(PLAYLIST_CATEGORIES_PATH);
    }

    savePlaylistBouquet(bouquet: PlaylistCategories): Observable<void> {
        return this.post<void>(PLAYLIST_BOUQUET_PATH, bouquet);
    }
}
