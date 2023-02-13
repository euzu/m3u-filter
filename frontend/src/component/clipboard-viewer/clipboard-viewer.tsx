import React, {useCallback, useEffect, useState} from 'react';
import './clipboard-viewer.scss';
import {noop, Observable} from "rxjs";
import {ContentCopy, DeleteSweep} from "@mui/icons-material";
import copyToClipboard from "../../utils/clipboard";
import {first} from "rxjs/operators";
import {useSnackbar} from "notistack";

interface ClipboardViewerProps {
    channel: Observable<string>;
}

export default function ClipboardViewer(props: ClipboardViewerProps): JSX.Element {

    const {channel} = props;
    const [data, setData] = useState<string[]>([]);
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();

    useEffect(() => {
        const sub = channel.subscribe({
            next: (value: string) => setData(d => [ ...d, value]),
        });
        return () => sub.unsubscribe();
    }, [channel]);

    const handleClear = useCallback(() => {
        setData([]);
    }, []);

    const handleCopyToClipboard = useCallback(() => {
        if (data?.length) {
            copyToClipboard(data.join('\n')).pipe(first()).subscribe({
                next: value => enqueueSnackbar(value ? "Copied to clipboard" : "Copy to clipboard failed!", {variant: value ? 'success' : 'error'}),
                error: err => enqueueSnackbar("Copy to clipboard failed!", {variant: 'error'}),
                complete: noop,
            });
        }
    }, [data, enqueueSnackbar]);

    return <div className={'clipboard-viewer'}>
        <div className={'clipboard-viewer-toolbar'}>
            <button className={'toolbar-btn'} onClick={handleClear}><DeleteSweep/></button>
            <button className={'toolbar-btn'} onClick={handleCopyToClipboard}><ContentCopy/></button>
        </div>
        <div className={'clipboard-viewer-content'}>
            <ul>
              {data.map((t, i) => <li key={'text-'+ i}>{t}</li>)}
            </ul>
        </div>
    </div>;
}