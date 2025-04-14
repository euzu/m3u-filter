import React, {JSX, useRef} from "react";
import VideoPlayer from "../video-player/video-player";
import {PlaylistCategories, PlaylistItem} from "../../model/playlist";
import {Observable} from "rxjs";
import {PlaylistRequest} from "../../model/playlist-request";
import ChannelView from "./channel-view";

interface PlaylistVideoProps {
    data: PlaylistCategories;
    channel: Observable<[PlaylistItem, PlaylistRequest]>;
    onPlay?: (playlistItem: PlaylistItem) => void;
}

export default function PlaylistVideo(props: PlaylistVideoProps): JSX.Element {
    const {data, channel, onPlay} = props;
    const playerRef = useRef(undefined);
    const handlePlayerReady = (player: any) => {
        playerRef.current = player;
    };

    return <div className={'playlist-video'}>
        <ChannelView data={data} onPlay={onPlay}></ChannelView>
        <VideoPlayer channel={channel} onReady={handlePlayerReady}/>
    </div>;
}