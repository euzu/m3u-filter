import {Observable} from "rxjs";
import ApiService, {DefaultApiService} from "./api-service";
import {UserPlaylistCategories} from "../model/playlist";

const PLAYLIST_CATEGORIES_PATH = "user/playlist/categories";
const PLAYLIST_BOUQUET_PATH = "user/playlist/bouquet";

export default interface UserConfigApiService extends ApiService {
    getPlaylistBouquet(): Observable<UserPlaylistCategories>;
    getPlaylistCategories(): Observable<UserPlaylistCategories>;
    savePlaylistBouquet(bouquet: UserPlaylistCategories): Observable<void>;
}

export class DefaultUserConfigApiService extends DefaultApiService implements UserConfigApiService {

    getPlaylistBouquet(): Observable<UserPlaylistCategories> {
        return this.get<UserPlaylistCategories>(PLAYLIST_BOUQUET_PATH);
    }
    getPlaylistCategories(): Observable<UserPlaylistCategories> {
        return this.get<UserPlaylistCategories>(PLAYLIST_CATEGORIES_PATH);
    }

    savePlaylistBouquet(bouquet: UserPlaylistCategories): Observable<void> {
        return this.post<void>(PLAYLIST_BOUQUET_PATH, bouquet);
    }
}
