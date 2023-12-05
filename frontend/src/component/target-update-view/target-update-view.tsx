import React, {useCallback, useMemo, useRef} from "react";
import './target-update-view.scss';
import ServerConfig from "../../model/server-config";
import ConfigUtils from "../../utils/config-utils";
import Checkbox from "../checkbox/checkbox";
import {useServices} from "../../provider/service-provider";
import {useSnackbar} from "notistack";

interface TargetUpdateViewProps {
    config: ServerConfig
}

export default function TargetUpdateView(props: TargetUpdateViewProps) {
    const {config} = props;

    const services = useServices();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const targets = useMemo(() => ConfigUtils.getTargetNames(config), [config]);
    const selected = useRef([]);

    const handleSelect = useCallback((checked: boolean, target: string) => {
        if (checked) {
            selected.current.push(target);
        } else {
            const idx = selected.current.indexOf(target);
            selected.current.slice(idx, 1);
        }
    }, []);

    const handleUpdate = useCallback((evt: any) => {
        services.playlist().update(selected.current).subscribe({
            next: () => enqueueSnackbar('Playlist update started', {variant: 'success'}),
            error: (err) => enqueueSnackbar('Failed to update:' + err, {variant: 'error'}),
        });
    }, [services, enqueueSnackbar]);

    return <div className={'target-update'}>
        <div className={'target-update__toolbar'}><label>Update</label><button title={'Update'} onClick={handleUpdate}>Start</button></div>
        <div className={'target-update__content'}>
            <ul>
                {targets.map(t => <li key={t}><Checkbox label={t} value={t} checked={false} onSelect={handleSelect}></Checkbox></li>)}
            </ul>
        </div>
    </div>
}