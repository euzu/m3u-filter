import {Observable} from "rxjs";
import PlaylistApiService, {DefaultPlaylistApiService} from "../api/playlist-api-service";
import {first} from "rxjs/operators";
import {PlaylistRequest} from "../model/playlist-request";
import {PlaylistCategories, PlaylistGroup, PlaylistItem, PlaylistResponse} from "../model/playlist";

const mergeCategory = (groups: PlaylistGroup[], channels: PlaylistItem[]) => {
    if (groups?.length  && channels?.length) {
        let unknown: PlaylistGroup = {
            id: 0,
            name: "Unknown",
            channels: [],
        };
        const groupMap = groups.reduce((acc: any, group: PlaylistGroup) => {
            acc[group.name] = group;
            group.channels = group.channels ?? [];
            return acc;
        }, {});
        channels.forEach(channel =>  {
            let group = groupMap[channel.group] ?? unknown;
            group.channels.push(channel);
        });
    }
}

export default class PlaylistService {

    constructor(private playlistApiService: PlaylistApiService = new DefaultPlaylistApiService()) {
    }

    getPlaylistCategories(req: PlaylistRequest): Observable<PlaylistCategories> {
        return new Observable((obs) =>
            this.playlistApiService.getPlaylist(req).pipe(first()).subscribe({
                next: (pl: PlaylistResponse) => {
                    if (pl) {
                        ['live', 'vod', 'series'].forEach(key => mergeCategory((pl.categories as any)?.[key], (pl.channels as any)?.[key]));
                        let cnt = 0;
                        let categories = pl.categories;
                        [categories.live, categories.vod, categories.series].filter(Boolean).flat().forEach(g => {
                            g.id = ++cnt;
                            g.channels?.forEach(c => c.id = ++cnt);
                        })
                        obs.next(categories);
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
