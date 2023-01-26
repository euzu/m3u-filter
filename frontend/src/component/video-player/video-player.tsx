import React, {useCallback, useEffect} from 'react';
import './video-player.scss';
import {Observable, Subscription} from "rxjs";
import {PlaylistItem} from "../../model/playlist";

interface VideoPlayerProps {
    channel: Observable<PlaylistItem>;
    onReady: (player: any) => void;
}

export const VideoPlayer = (props: VideoPlayerProps) => {
    const {channel} = props;

    const handlePlayVideo = useCallback((playlistItem: PlaylistItem) => {
    }, []);

    useEffect(() => {
        let sub: Subscription = undefined;
        if (channel) {
            sub = channel.subscribe({next: handlePlayVideo});
        }
        return () => sub && sub.unsubscribe();
    }, [channel, handlePlayVideo]);

    return <React.Fragment />;
}

export default VideoPlayer;