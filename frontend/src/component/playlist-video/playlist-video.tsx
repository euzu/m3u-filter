import React, {useRef} from "react";
import VideoPlayer from "../video-player/video-player";
import {PlaylistItem} from "../../model/playlist";
import {Observable} from "rxjs";

interface PlaylistVideoProps {
    channel: Observable<PlaylistItem>;
}

export default function PlaylistVideo(props: PlaylistVideoProps): JSX.Element {
    const {channel} = props;
    const playerRef = useRef(null);
    const handlePlayerReady = (player: any) => {
        playerRef.current = player;
    };

    return <VideoPlayer channel={channel} onReady={handlePlayerReady}/>;
}