import {Observable} from "rxjs";
import PlaylistApiService, {DefaultPlaylistApiService} from "../api/playlist-api-service";
import {first} from "rxjs/operators";
import {PlaylistRequest} from "../model/playlist-request";
import {PlaylistCategories, PlaylistGroup, PlaylistItem} from "../model/playlist";

// const mergeCategory = (groups: PlaylistGroup[], channels: PlaylistItem[]) => {
//     if (groups?.length  && channels?.length) {
//         let unknown: PlaylistGroup = {
//             id: 0,
//             name: "Unknown",
//             channels: [],
//         };
//         const groupMap = groups.reduce((acc: any, group: PlaylistGroup) => {
//             acc[group.name] = group;
//             group.channels = group.channels ?? [];
//             return acc;
//         }, {});
//         channels.forEach(channel =>  {
//             let group = groupMap[channel.group] ?? unknown;
//             group.channels.push(channel);
//         });
//     }
// }

export default class PlaylistService {

    constructor(private playlistApiService: PlaylistApiService = new DefaultPlaylistApiService()) {
    }

    getPlaylistCategories(req: PlaylistRequest): Observable<PlaylistCategories> {
        return new Observable((obs) =>
            this.playlistApiService.getPlaylist(req).pipe(first()).subscribe({
                next: (pl: PlaylistCategories) => {
                    if (pl) {
                        let cnt = 0;
                        [pl?.live, pl?.vod, pl?.series].filter(Boolean).flat().forEach((g: PlaylistGroup) => {
                            g.id = ++cnt;
                            g.name = g.name ?? (g as any).title;
                            g?.channels?.forEach(c => {
                                c.id = ++cnt;
                            });
                        });
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

    getWebPlayerUrl(item: PlaylistItem, req: PlaylistRequest): Observable<string> {
        return this.playlistApiService.getWebPlayerUrl(item, req);
    }

}
