import {Observable} from "rxjs";
import PlaylistApiService, {DefaultPlaylistApiService} from "../api/playlist-api-service";
import {first} from "rxjs/operators";
import {PlaylistRequest} from "../model/playlist-request";
import {PlaylistCategories} from "../model/playlist";

export default class PlaylistService {

    constructor(private playlistApiService: PlaylistApiService = new DefaultPlaylistApiService()) {
    }

    getPlaylistCategories(req: PlaylistRequest): Observable<PlaylistCategories> {
        return new Observable((obs) =>
            this.playlistApiService.getPlaylist(req).pipe(first()).subscribe({
                next: (pl: PlaylistCategories) => {
                    if (pl) {
                        let cnt = 0;
                        [pl.live, pl.vod, pl.series].flat().forEach(g => {
                            g.id = ++cnt;
                            g.channels?.forEach(c => c.id = ++cnt);
                        })
                        obs.next(pl);
                    } else {
                        obs.error("Could not download playlist");
                    }
                },
                error: (e) => obs.error(e),
                complete: () => obs.complete(),
            }));
    }

    update(targets: string[]): Observable<any> {
        return this.playlistApiService.updateTargets(targets);
    }
}
