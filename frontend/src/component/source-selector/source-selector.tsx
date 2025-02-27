import React, {KeyboardEvent, useCallback, useRef} from "react";

import './source-selector.scss';
import {getIconByName} from "../../icons/icons";
import ServerConfig from "../../model/server-config";
import {PlaylistRequest, PlaylistRequestType} from "../../model/playlist-request";
import InputField from "../input-field/input-field";
import SourceSelectorEditor, {ISourceSelectorEditor,} from "../source-selector-editor/source-selector-editor";

const formatSourceSelection = (req: PlaylistRequest): string => {
    switch (req?.rtype) {
        case PlaylistRequestType.TARGET: {
            return `Target: ${req.sourceName}`;
        }
        case PlaylistRequestType.INPUT: {
            return `Provider: ${req.sourceName}`;
        }
        case PlaylistRequestType.XTREAM: {
            return `${req.url}/player_api.php?username=${req.username}&password=${req.password}`;
        }
        case PlaylistRequestType.M3U: {
            return req.url;
        }
        default:
            return '';
    }
};

interface SourceSelectorProps {
    serverConfig: ServerConfig;
    onDownload: (req: PlaylistRequest) => void;
}

export default function SourceSelector(props: SourceSelectorProps) {
    const editorRef = useRef<ISourceSelectorEditor>(undefined);
    const textFieldRef = useRef<HTMLInputElement>(undefined);
    const playlistRequestRef = useRef<PlaylistRequest>(undefined);

    const {serverConfig, onDownload} = props;

    const handleDownload = useCallback(() => {
        if (playlistRequestRef.current) {
            onDownload(playlistRequestRef.current);
        }
    }, [onDownload]);

    const handleKeyPress = useCallback((event: KeyboardEvent<any>) => {
        if (event.key === 'Enter') {
            handleDownload();
        }
    }, [handleDownload]);

    const openPopup = useCallback((evt: any) => {
        editorRef.current.open();
    }, []);


    const handleEditorSubmit = useCallback((req: PlaylistRequest): boolean => {
        textFieldRef.current.value = formatSourceSelection(req);
        playlistRequestRef.current = req;
        return true;
    }, []);

    return <div className={'source-selector'}>
        <InputField label={'Source'}>
            <input readOnly={true} onKeyUp={handleKeyPress} ref={textFieldRef}/>
            <button data-tooltip={'Download'} onClick={handleDownload}>{getIconByName('CloudDownload')}</button>
            <button data-tooltip={'Input List'} onClick={openPopup}>{getIconByName('ArrowDown')}</button>
        </InputField>
        <SourceSelectorEditor onSubmit={handleEditorSubmit} ref={editorRef}
                              serverConfig={serverConfig}></SourceSelectorEditor>
    </div>
}