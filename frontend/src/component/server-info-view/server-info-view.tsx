import React, {useCallback, useMemo} from "react";
import './server-info-view.scss';
import ServerConfig, {ServerInfo} from "../../model/server-config";
import {useSnackbar} from "notistack";
import {useServices} from "../../provider/service-provider";

const isNumber = (value: string): boolean => {
    return false;
}

const SERVER_INFO_FIELDS = [
    {name: 'protocol', caption: 'Protocol', options: ['http', 'https']},
    {name: 'ip', caption: 'IP'},
    {name: 'http_port', caption: 'HTTP port', validator: isNumber},
    {name: 'https_port', caption: 'HTTPS port', validator: isNumber},
    {name: 'rtmp_port', caption: 'RTMP port', validator: isNumber},
    {name: 'timezone', caption: 'Timezone'},
    {name: 'message', caption: 'Message'},
];

interface ServerInfoViewProps {
    config: ServerConfig;
}

export default function ServerInfoView(props: ServerInfoViewProps) {
    const {config} = props;
    const services = useServices();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const serverInfo = useMemo<ServerInfo>(() => config?.api_proxy?.server, [config]);

    const handleValueChange = useCallback((evt: any) => {
        const field = evt.target.dataset.field;
        if (serverInfo) {
            (serverInfo as any)[field] = evt.target.value;
        }
    }, [serverInfo]);

    const handleSave = useCallback(() => {
        if (serverInfo) {
            // @TODO validations
            services.config().saveServerInfo(serverInfo).subscribe({
                next: () => enqueueSnackbar("Server Info saved!", {variant: 'success'}),
                error: (err) => enqueueSnackbar("Failed to save server info!", {variant: 'error'})
            });
        }
    }, [services, serverInfo, enqueueSnackbar]);

    return <div className={'server-info'}>
        <div className={'server-info__toolbar'}><label>Server</label>
            <button title={'Save'} onClick={handleSave}>Save</button>
        </div>
        <div className={'server-info__content'}>
            <div className={'server-info__content-table'}>
                {
                    SERVER_INFO_FIELDS.map(field =>
                        <div key={'server_info_field_' + field.name} className={'server-info__content-row'}>
                            <div className={'server-info__content-col'}>
                                <label>{field.caption}</label>
                            </div>
                            <div className={'server-info__content-table-col'}>
                                <input defaultValue={(serverInfo as any)?.[field.name]} data-field={field.name}
                                       onChange={handleValueChange}></input>
                            </div>
                        </div>
                    )
                }
            </div>
        </div>
    </div>
}