import React, {useCallback, useMemo, useRef} from "react";
import './target-update-view.scss';
import ServerConfig from "../../model/server-config";
import ConfigUtils from "../../utils/config-utils";
import Checkbox from "../checkbox/checkbox";
import {useServices} from "../../provider/service-provider";
import {useSnackbar} from "notistack";
import useTranslator from "../../hook/use-translator";

interface TargetUpdateViewProps {
    config: ServerConfig
}

export default function TargetUpdateView(props: TargetUpdateViewProps) {
    const {config} = props;

    const services = useServices();
    const translate = useTranslator();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const targets = useMemo(() => ConfigUtils.getTargetNames(config), [config]);
    const selected = useRef([]);

    const handleSelect = useCallback((target: string,checked: boolean) => {
        if (checked) {
            selected.current.push(target);
        } else {
            const idx = selected.current.indexOf(target);
            selected.current.splice(idx, 1);
        }
    }, []);

    const handleUpdate = useCallback((evt: any) => {
        services.playlist().update(selected.current).subscribe({
            next: () => enqueueSnackbar(translate('MESSAGES.PLAYLIST_UPDATE.SUCCESS'), {variant: 'success'}),
            error: (err) => enqueueSnackbar(translate('MESSAGES.PLAYLIST_UPDATE.FAILED') + err, {variant: 'error'}),
        });
    }, [services, enqueueSnackbar, translate]);

    return <div className={'target-update'}>
        <div className={'target-update__toolbar'}><label>{translate('LABEL.UPDATE')}</label><button title={'Update'} onClick={handleUpdate}>{translate('LABEL.START')}</button></div>
        <div className={'target-update__content'}>
            <ul>
                {targets.map(t => <li key={t}><Checkbox label={t} value={t} checked={false} onSelect={handleSelect}></Checkbox></li>)}
            </ul>
        </div>
    </div>
}