import {Observable} from "rxjs";
import PlaylistApiService, {DefaultPlaylistApiService} from "../api/playlist-api-service";
import {PlaylistGroup} from "../model/playlist";
import {first} from "rxjs/operators";

export default class PlaylistService {

    constructor(private playlistApiService: PlaylistApiService = new DefaultPlaylistApiService()) {
    }

    getPlaylist(url: string): Observable<PlaylistGroup[]> {
        return new Observable((obs) =>
            this.playlistApiService.getPlaylist(url).pipe(first()).subscribe({
                next: (pl) => {
                    let cnt = 0;
                    pl.forEach(g => {
                        g.id = ++cnt;
                        g.channels.forEach(c => c.id = ++cnt);
                    })
                    obs.next(pl);
                },
                error: (e) => obs.error(e),
                complete: () => obs.complete(),
            }));
    }
}
