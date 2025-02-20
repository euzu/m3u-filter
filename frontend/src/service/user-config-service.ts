import { Observable } from "rxjs";
import UserConfigApiService, {DefaultUserConfigApiService} from "../api/user-config-api-service";
import {PlaylistCategories} from "../model/playlist-categories";
import {PlaylistBouquet} from "../model/playlist-bouquet";

export default class UserConfigService {
    constructor(private userConfigApiService: UserConfigApiService = new DefaultUserConfigApiService()) {
    }

    getPlaylistBouquet(): Observable<PlaylistBouquet> {
        return this.userConfigApiService.getPlaylistBouquet();
    }
    getPlaylistCategories(): Observable<PlaylistCategories> {
        return this.userConfigApiService.getPlaylistCategories();
    }

    savePlaylistBouquet(bouquet: PlaylistCategories): Observable<void> {
        return this.userConfigApiService.savePlaylistBouquet(bouquet);
    }
}
