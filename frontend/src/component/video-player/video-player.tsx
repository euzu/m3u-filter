import React, {useCallback, useEffect, useRef} from 'react';
import './video-player.scss';
import {first, Observable, Subscription} from "rxjs";
import {PlaylistItem, PlaylistItemType} from "../../model/playlist";
import videojs from "video.js";
import 'videojs-mpegtsjs';
import "video.js/dist/video-js.css";
import {useServices} from '../../provider/service-provider';
import {PlaylistRequest, PlaylistRequestType} from "../../model/playlist-request";

const DEFAULT_OPTIONS: any = {
    autoplay: true,
    controls: true,
    responsive: true,
    fluid: true,
    fill: true,
    sources: [],
    poster: undefined,
    preload: "none",
};

const MPEGTS_OPTIONS: any = {
    mediaDataSource: {
        type: 'mpegts',
        isLive: true,
        cors: true,
        withCredentials: false,
    },
    config: {
        enableWorker: true,
        enableWorkerForMSE: true,
        enableStashBuffer: true,
    }
};

// @ts-ignore
type Player = videojs.Player | null;

interface VideoPlayerProps {
    channel: Observable<[PlaylistItem, PlaylistRequest]>;
    onReady?: (player: any) => void;
}

export const VideoPlayer = ({channel, onReady}: VideoPlayerProps) => {
    const services = useServices();
    const videoRef = useRef<HTMLDivElement | null>(null);
    const playerRef = useRef<Player>(null);

    const playVideo = useCallback((playlistItem: PlaylistItem, url: string) => {
        const isLive = [
            PlaylistItemType.Live,
            PlaylistItemType.LiveHls,
            PlaylistItemType.LiveUnknown,
            PlaylistItemType.Catchup
        ].includes(playlistItem.item_type);

        // TODO mimetype
        const playerOptions = {
            ...DEFAULT_OPTIONS,
            sources: [{
                src: url,
                type: isLive ? 'video/mp2t' : 'video/mp4',
            }],
            poster: playlistItem.logo ?? playlistItem.logo_small,
            mpegtsjs: isLive ? {
                ...MPEGTS_OPTIONS,
                mediaDataSource: {...MPEGTS_OPTIONS.mediaDataSource, url}
            } : undefined,
        };

        try {

            if (playerRef.current && playerRef.current.currentSrc() !== playerOptions.sources[0].src) {
                const player = playerRef.current;
                player.options(playerOptions);
                player.poster(playerOptions.poster);
                player.src(playerOptions.sources);
            }
        } catch (e) {
            console.error(e);
        }
    }, [])

    const handlePlayVideo = useCallback(([playlistItem, playlistRequest]: [playlistItem: PlaylistItem, playlistRequest: PlaylistRequest]) => {
        switch (playlistRequest.rtype) {
            case PlaylistRequestType.TARGET:
                services.playlist().getReverseUrl(playlistItem, playlistRequest).pipe(first()).subscribe({
                    next: (url: string) => {
                        switch (playlistRequest.rtype) {
                        }
                        playVideo(playlistItem, url);
                    },
                    error: (error: any) => {
                        playVideo(playlistItem, playlistItem.url);
                    },
                });
                break;
            case PlaylistRequestType.INPUT:
            case PlaylistRequestType.XTREAM:
            case PlaylistRequestType.M3U:
                playVideo(playlistItem, playlistItem.url);
                break;
        }
    }, [services, playVideo]);

    useEffect(() => {
        const sub: Subscription = channel.subscribe({
            next: handlePlayVideo,
        });
        return () => sub.unsubscribe();
    }, [channel, handlePlayVideo]);

    useEffect(() => {
        if (!playerRef.current && videoRef.current) {
            const videoElement = document.createElement("video-js");
            videoElement.classList.add("vjs-big-play-centered");
            videoElement.setAttribute('controls', 'true');
            videoRef.current.appendChild(videoElement);

            const player = videojs(videoElement, {}, () => {
                videojs.log("player is ready");
                onReady && onReady(player);
            });

            playerRef.current = player;
            // Error handling
            player.on('error', () => {
                const error = player.error();
                console.error('Video.js Error:', error);

                if (error && error.code === 4) {
                    console.warn('Not found.');
                }
            });

            // const mpegtsPlayer = (player.tech(true) as any).flvPlayer;
            // if (mpegtsPlayer) {
            //     mpegtsPlayer.on(mpegts.Events.ERROR, (type, details) => {
            //         console.error("mpegts.js ERROR", type, details);
            //     });
            // }
        }
    }, [onReady]);

    useEffect(() => {
        return () => {
            if (playerRef.current && !playerRef.current.isDisposed()) {
                playerRef.current.dispose();
                playerRef.current = null;
            }
        };
    }, []);

    return (
        <div data-vjs-player className="video-player">
            <div ref={videoRef}/>
        </div>
    );
}

export default VideoPlayer;
