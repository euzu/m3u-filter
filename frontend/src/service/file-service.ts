import {PlaylistItem, PlaylistGroup} from "../model/playlist";
import FileSaver from "file-saver";

export default class FileService {
    save(playlist: PlaylistGroup[]) {
        const lines = ['#EXTM3U'];
        playlist.forEach(group => {
             group.channels.forEach((entry: PlaylistItem) => {
                 lines.push(entry.header.source);
                 lines.push(entry.url);
             })
        });
        const blob = new Blob([lines.join('\n')], { type: "text/plain;charset=utf-8" });
        FileSaver.saveAs(blob, "playlist.m3u");
    }
}