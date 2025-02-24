import React, {useCallback, useEffect, useMemo, useRef, useState} from 'react';
import './app.scss';
import SourceSelector from "../component/source-selector/source-selector";
import PlaylistViewer, {IPlaylistViewer, SearchRequest} from "../component/playlist-viewer/playlist-viewer";
import {useSnackbar} from 'notistack';
import Toolbar from "../component/toolbar/toolbar";
import {PlaylistGroup, PlaylistItem} from "../model/playlist";
import {useServices} from "../provider/service-provider";
import {first} from "rxjs/operators";
import Progress from '../component/progress/progress';
import PlaylistFilter from "../component/playlist-filter/playlist-filter";
import {noop, Subject} from "rxjs";
import PlaylistVideo from "../component/playlist-video/playlist-video";
import ClipboardViewer from "../component/clipboard-viewer/clipboard-viewer";
import Sidebar from "../component/sidebar/sidebar";
import {PlaylistRequest} from "../model/playlist-request";
import ServerConfig from "../model/server-config";
import {getIconByName} from "../icons/icons";
import Preferences from "../component/preferences/preferences";
import FileDownload from "../component/file-download/file-download";
import {FileDownloadInfo} from "../model/file-download";
import useTranslator, {LANGUAGE} from "../hook/use-translator";
import i18next from "i18next";

/* eslint-disable @typescript-eslint/no-empty-interface */
interface AppProps {

}

export default function App(props: AppProps) {
    const searchChannel = useMemo<Subject<SearchRequest>>(() => new Subject<SearchRequest>(), []);
    const services = useServices();
    const [progress, setProgress] = useState<boolean>(false);
    const [playlist, setPlaylist] = useState<PlaylistGroup[]>([]);
    const [serverConfig, setServerConfig] = useState<ServerConfig>(undefined);
    const serverConfigLoaded = useRef(false);
    const [preferencesVisible, setPreferencesVisible] = useState<boolean>(true);
    const clipboardChannel = useMemo<Subject<string>>(() => new Subject<string>(), []);
    const viewerRef = useRef<IPlaylistViewer>(undefined);
    const appTitle = useMemo(() => services.config().getUiConfig().app_title ?? 'm3u-filter', [services]);
    const translate = useTranslator();
    const appLogo = useMemo(() => {
        let logo =  services.config().getUiConfig().app_logo;
        if (logo) {
            return <img src={logo} alt="logo" />;
        } else {
            return getIconByName('Logo');
        }
    }, [services]);
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const videoChannel = useMemo(() => new Subject<PlaylistItem>(), []);
    const handleDownload = useCallback((req: PlaylistRequest) => {
        setProgress(true);
        services.playlist().getPlaylist(req).pipe(first()).subscribe({
            next: (pl: PlaylistGroup[]) => {
                enqueueSnackbar(translate( "MESSAGES.DOWNLOAD.PLAYLIST.SUCCESS"), {variant: 'success'})
                setPlaylist(pl);
            },
            error: (err) => {
                setProgress(false);
                enqueueSnackbar(translate( "MESSAGES.DOWNLOAD.PLAYLIST.FAIL"), {variant: 'error'});
            },
            complete: () => setProgress(false),
        });
    }, [enqueueSnackbar, services, translate]);

    const handleSave = useCallback(() => {
        const filteredPlaylist = viewerRef.current.getFilteredPlaylist();
        if (filteredPlaylist?.length) {
            services.file().save(filteredPlaylist);
        }
    }, [services]);

    const handleFilter = useCallback((filter: string, regexp: boolean): void => {
        searchChannel.next({filter, regexp});
    }, [searchChannel]);

    const handleProgress = useCallback((value: boolean) => {
        setProgress(value);
    }, []);

    const handleOnPlay = useCallback((playlistItem: PlaylistItem): void => {
        videoChannel.next(playlistItem);
    }, [videoChannel]);

    const handleOnCopy = useCallback((playlistItem: PlaylistItem): void => {
        clipboardChannel.next(playlistItem.header.url);
    }, [clipboardChannel]);

    const handleOnDownload = useCallback((playlistItem: PlaylistItem): void => {
        let filename = playlistItem.header.title;
        const parts = playlistItem.header.url.split('.');
        let ext = undefined;
        if (parts.length > 1) {
            ext = parts[parts.length - 1];
        }

        if (serverConfig.video?.extensions?.includes(ext)) {
            services.file().download({
                url: playlistItem.header.url,
                filename: filename + '.' + ext
            }).pipe(first()).subscribe({
                next: (_: FileDownloadInfo) => {
                },
                error: _ => enqueueSnackbar(translate( "MESSAGES.DOWNLOAD.FAIL"), {variant: 'error'}),
                complete: noop,
            });
        } else {
            enqueueSnackbar(translate( "MESSAGES.INVALID_FILETYPE"), {variant: 'error'})
        }
    }, [serverConfig, enqueueSnackbar, services, translate]);

    const handleOnWebSearch = useCallback((playlistItem: PlaylistItem): void => {
        if (playlistItem) {
            let title = playlistItem.header.title;
            let pattern = serverConfig.video.download?.episode_pattern;
            if (pattern) {
                pattern = pattern.replace('?P<episode>', '?<episode>');
                const matches = title.match(pattern);
                if (matches?.groups?.episode) {
                    const idx = title.indexOf(matches?.groups?.episode);
                    title = title.substring(0, idx).trim();
                }
            }
            const dateSuffixMatch = title.match(/(.*?).\(\d+\)/);
            if (dateSuffixMatch?.length > 1) {
                title = dateSuffixMatch[1];
            }

            const url = serverConfig.video.web_search.replace("{}", title);
            window.open(url, "imdb");
        }
    }, [serverConfig]);

    useEffect(() => {
        if (!serverConfigLoaded.current) {
            serverConfigLoaded.current = true;
            services.config().getServerConfig().pipe(first()).subscribe({
                next: (cfg: ServerConfig) => {
                    setServerConfig(cfg);
                },
                error: (err) => {
                    enqueueSnackbar(translate("MESSAGES.DOWNLOAD.SERVER_CONFIG.FAIL"), {variant: 'error'});
                },
                complete: noop,
            });
        }
        return noop
    }, [enqueueSnackbar, services, translate]);

    const handlePreferences = useCallback(() => {
       setPreferencesVisible((value:boolean) => !value);
    }, []);

    const handleLogout = useCallback(() => {
        services.auth().logout();
    }, [services]);

    const handleLanguage = useCallback((event: any) => {
        const language = event.target.value;
        LANGUAGE.next(language);
    }, []);

    return (
        <div className="app">
            <div className={'app-header'}>
                <div className={'app-header__caption'}><span className={'app-header__logo'}>{appLogo}</span>{appTitle}</div>
                <div className={'app-header__toolbar'}><select onChange={handleLanguage}>{services.config().getUiConfig().languages.map(l => <option key={l} value={l} selected={l === i18next.language}>{l}</option>)}</select></div>
                <div className={'app-header__toolbar'}><button title={translate('LABEL.CONFIGURATION')} onClick={handlePreferences}>{getIconByName('Config')}</button></div>
                <div className={'app-header__toolbar'}><button title={translate('LABEL.LOGOUT')} onClick={handleLogout}>{getIconByName('Logout')}</button></div>
            </div>
            <div className={'app-main' + (preferencesVisible ? '' : '  hidden')}>
                <div className={'app-content'}>
                    <Preferences config={serverConfig} />
                </div>
            </div>
            <div className={'app-main' + (preferencesVisible ? ' hidden' : '')}>
                <Sidebar>
                    <ClipboardViewer channel={clipboardChannel}></ClipboardViewer>
                </Sidebar>
                <div className={'app-content'}>
                    <SourceSelector onDownload={handleDownload} serverConfig={serverConfig}/>
                    <PlaylistFilter onFilter={handleFilter}/>
                    <PlaylistViewer ref={viewerRef} playlist={playlist} searchChannel={searchChannel}
                                    onProgress={handleProgress} onCopy={handleOnCopy} onPlay={handleOnPlay}
                                    onDownload={handleOnDownload}
                                    onWebSearch={handleOnWebSearch}
                                    serverConfig={serverConfig}/>
                    <PlaylistVideo channel={videoChannel}/>
                    <Toolbar onDownload={handleSave}/>
                    <FileDownload></FileDownload>
                    <Progress visible={progress}/>
                </div>
            </div>
        </div>
    );
}
