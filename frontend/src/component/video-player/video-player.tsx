import React, {useCallback, useEffect, useRef} from 'react';
import './video-player.scss';
import {first, Observable, Subscription} from "rxjs";
import {PlaylistItem, PlaylistItemType} from "../../model/playlist";
import videojs from "video.js";
import 'videojs-mpegtsjs';
import "video.js/dist/video-js.css";
import {useServices} from '../../provider/service-provider';
import {PlaylistRequest, PlaylistRequestType} from "../../model/playlist-request";

const VIDEOJS_OPTIONS: any = {
    autoplay: true,
    controls: true,
    responsive: false,
    fluid: true,
    preload: "none",
}

const DEFAULT_OPTIONS: any = {
    ...VIDEOJS_OPTIONS,
    sources: [],
    poster: undefined,
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

const HLS_OPTIONS: any = {
    html5: {
        vhs: {
            cors: true,
            overrideNative: true
        },
        nativeAudioTracks: false,
        nativeVideoTracks: false
    }
};

// @ts-ignore
type Player = videojs.Player | null;

const getVideoOptions = (playlistItem: PlaylistItem, url: string): { mimeType: string, options: any } => {
    switch (playlistItem.item_type) {
        case PlaylistItemType.Video:
        case PlaylistItemType.Series:
            return {mimeType: 'video/mp4', options: {}};
        case PlaylistItemType.SeriesInfo:
            return {mimeType: 'application/json', options: {}};
        case PlaylistItemType.Live:
            return {
                mimeType: 'video/mp2t', options: {
                    mpegtsjs: {
                        ...MPEGTS_OPTIONS,
                        mediaDataSource: {...MPEGTS_OPTIONS.mediaDataSource, url}
                    }
                }
            };
        case PlaylistItemType.Catchup:
        case PlaylistItemType.LiveUnknown:
        case PlaylistItemType.LiveHls:
            return {mimeType: 'application/x-mpegURL', options: HLS_OPTIONS};
    }
    return {mimeType: 'application/octet-stream', options: {}};
}

interface VideoPlayerProps {
    channel: Observable<[PlaylistItem, PlaylistRequest]>;
    onReady?: (player: any) => void;
}

export const VideoPlayer = ({channel, onReady}: VideoPlayerProps) => {
    const services = useServices();
    const videoRef = useRef<HTMLDivElement | null>(null);
    const playerRef = useRef<Player>(null);

    const playVideo = useCallback((playlistItem: PlaylistItem, url: string) => {
        let options = getVideoOptions(playlistItem, url);
        // TODO mimetype
        const playerOptions = {
            ...DEFAULT_OPTIONS,
            sources: [{
                src: url,
                type: options.mimeType,
            }],
            poster: playlistItem.logo ?? playlistItem.logo_small,
            ...options.options
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
                if (playlistItem.item_type === PlaylistItemType.LiveHls) {
                    playVideo(playlistItem, playlistItem.url);
                } else {
                    services.playlist().getWebPlayerUrl(playlistItem, playlistRequest).pipe(first()).subscribe({
                        next: (url: string) => {
                            playVideo(playlistItem, url);
                        },
                        error: (error: any) => {
                            playVideo(playlistItem, playlistItem.url);
                        },
                    });
                }
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
            videoElement.setAttribute('controls', VIDEOJS_OPTIONS.controls ?? 'false');
            videoElement.setAttribute('autoplay', VIDEOJS_OPTIONS.autoplay ?? 'false');
            videoElement.setAttribute('responsive', VIDEOJS_OPTIONS.responsive ?? 'false');
            //videoElement.setAttribute('fluid', VIDEOJS_OPTIONS.fluid ?? 'false');
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
        <div ref={videoRef} data-vjs-player className="video-player">
        </div>
    );
}

export default VideoPlayer;
