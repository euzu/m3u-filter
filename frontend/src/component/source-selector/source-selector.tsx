import React, {KeyboardEvent, useCallback, useEffect, useRef, useState} from "react";

import './source-selector.scss';
import {getIconByName} from "../../icons/icons";
import ServerConfig, {InputConfig} from "../../model/server-config";
import PopupMenu from "../popup-menu/popup-menu";
import {PlaylistRequest} from "../../model/playlist-request";
import InputField from "../input-field/input-field";

interface SourceSelectorProps {
    serverConfig: ServerConfig;
    onDownload: (req: PlaylistRequest) => void;
}

export default function SourceSelector(props: SourceSelectorProps) {
    const textField = useRef<HTMLInputElement>();
    const [popupVisible, setPopupVisible] = useState<{ x: number, y: number }>(undefined);
    const [sources, setSources] = useState<InputConfig[]>([]);
    const [selected, setSelected] = useState<InputConfig>(undefined);

    const {serverConfig, onDownload} = props;

    const handleDownload = useCallback(() => {
        const value = textField.current.value;
        if (value && value.trim().length > 0) {
            if (value.trim() == selected?.name) {
                onDownload({input_id: selected.id});
            } else {
                onDownload({url: value.trim()});
            }
        }
    }, [onDownload, selected]);

    const handleKeyPress = useCallback((event: KeyboardEvent<any>) => {
        if (event.key === 'Enter') {
            handleDownload();
        }
    }, [handleDownload]);

    const closePopup = useCallback(() => {
        setPopupVisible(undefined);
    }, []);

    const openPopup = useCallback((evt: any) => {
        const rect = evt.target.getBoundingClientRect();
        const top = rect.y + rect.height;
        setPopupVisible({x: evt.clientX, y: top});
    }, []);

    const handleMenuClick = useCallback((evt: any) => {
        const idx = evt.target.dataset.idx;
        if (idx != null) {
            setPopupVisible(undefined);
            setSelected(sources[idx]);
            textField.current.value = sources[idx].name || sources[idx].url;
        }
    }, [sources]);

    useEffect(()=> {
        if (serverConfig) {
            setSources(serverConfig.sources?.flatMap(source => source.inputs))
        }
    }, [serverConfig]);

    return <div className={'source-selector'}>
        <InputField label={'Source'}>
            <input onKeyUp={handleKeyPress} ref={textField}/>
            <button onClick={handleDownload}>{getIconByName('CloudDownload')}</button>
            <button onClick={openPopup}>{getIconByName('ArrowDown')}</button>
        </InputField>
        <PopupMenu position={popupVisible} onHide={closePopup}>
            <ul>
                {sources.map((s, idx) =>
                    <li key={s.id + '_' + idx} data-idx={idx} onClick={handleMenuClick}>{s.name || s.url}</li>)}
            </ul>
        </PopupMenu>
    </div>
}
