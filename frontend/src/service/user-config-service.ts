import { Observable } from "rxjs";
import UserConfigApiService, {DefaultUserConfigApiService} from "../api/user-config-api-service";
import {UserPlaylistCategories} from "../model/playlist";

export default class UserConfigService {
    constructor(private userConfigApiService: UserConfigApiService = new DefaultUserConfigApiService()) {
    }

    getPlaylistBouquet(): Observable<UserPlaylistCategories> {
        return this.userConfigApiService.getPlaylistBouquet();
    }
    getPlaylistCategories(): Observable<UserPlaylistCategories> {
        return this.userConfigApiService.getPlaylistCategories();
    }

    savePlaylistBouquet(bouquet: UserPlaylistCategories): Observable<void> {
        return this.userConfigApiService.savePlaylistBouquet(bouquet);
    }
}
