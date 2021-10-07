import React, {useRef, useState, useEffect, useCallback, useMemo} from "react";

import './source-selector.scss';
import {IconButton, InputAdornment, Menu, MenuItem, TextField} from "@material-ui/core";
import {CloudDownload, ArrowDropDown} from "@material-ui/icons";
import Source from "../../model/source";
import {useServices} from "../../provider/service-provider";
import {first} from "rxjs/operators";
import {useSnackbar} from "notistack";
import {noop} from "rxjs";

interface SourceSelectorProps {
    onDownload: (url: string) => void;
}

export default function SourceSelector(props: SourceSelectorProps) {
    const textField = useRef<HTMLInputElement>();
    const services = useServices();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const [openSourceDropDown, setOpenSourceDropDown] = useState(false);
    const [sources, setSources] = useState<Source[]>([]);
    const [source, setSource] = useState<Source>(null);

    const {onDownload} = props;

    const addNewSource = useCallback((url: string) => {
        const src: Source = {
            url,
            ts: Date.now()
        }
        if (source) {
            if (source?.url !== url) {
                setSources((s) => [].concat(s, [src]))
            }
        } else {
            setSources([src]);
        }
    }, [source]);

    const inputAddNewSource = useCallback((evt: any) => {
        addNewSource(evt.target.value);
    }, [addNewSource])

    const handleDownload = useCallback(() => {
        const value = textField.current.value;
        if (value && value.trim().length > 0) {
            addNewSource(value);
            onDownload(value);
        }
    }, [addNewSource, onDownload]);

    const selectSource = useCallback((src: Source) => {
        if (src) {
            setSource(src);
        }
    }, []);

    const closeSourceDropDown = useCallback(() => {
        setOpenSourceDropDown(false);
    }, []);

    const openSourcesDropDown = useCallback(() => {
        setOpenSourceDropDown(true);
    }, []);

    const selectSourceDropDown = useCallback((e: any) => {
        setOpenSourceDropDown(false);
        const url = e.target.dataset.url;
        selectSource({url} as Source);
    }, [selectSource]);

    useEffect(() => {
        services.config().getServerConfig().pipe(first()).subscribe({
            next: (cfg) => {
                setSources(cfg.sources?.map(s => ({url: s, ts: Date.now()} as Source)))
            },
            error: (err) => {
                enqueueSnackbar('Failed to download server config!', {variant: 'error'});
            },
            complete: noop,
        });
        return noop
    }, [enqueueSnackbar, services]);

    const inputLabelProps = useMemo(() => ({
        shrink: true,
    }), []);

    const inputProps = useMemo(() => (
        {
            value: source?.url,
            onChange: inputAddNewSource,
            endAdornment: <InputAdornment position="end">
                <IconButton
                    className={"icon-button"}
                    aria-label="download"
                    onClick={handleDownload}
                    edge="end">
                    <CloudDownload/>
                </IconButton>
                <IconButton
                    className={"icon-button"}
                    aria-label="select"
                    onClick={openSourcesDropDown}
                    edge="end">
                    <ArrowDropDown/>
                </IconButton>
            </InputAdornment>
        }
    ), [handleDownload, openSourcesDropDown, inputAddNewSource, source]);

    return <div className={'source-selector'}>
        <TextField className={'source-selector-input'} inputRef={textField} label="Url" variant="outlined"
                   InputLabelProps={inputLabelProps}
                   InputProps={inputProps}
        />
        <Menu
            id="source-menu"
            anchorEl={textField.current}
            keepMounted
            open={openSourceDropDown}
            onClose={closeSourceDropDown}
        >
            {sources?.map(s =>
                <MenuItem key={s.ts} data-url={s.url} onClick={selectSourceDropDown}>{s.url}</MenuItem>)
            }
        </Menu>
    </div>
}
