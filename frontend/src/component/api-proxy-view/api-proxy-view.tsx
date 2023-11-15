import React, {useCallback, useMemo} from "react";
import './api-proxy-view.scss';
import ServerConfig, {ServerInfo} from "../../model/server-config";
import {useSnackbar} from "notistack";
import {useServices} from "../../provider/service-provider";

const isNumber = (value: string): boolean => {
    return !isNaN(value as any);
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

interface ApiProxyViewProps {
    config: ServerConfig;
}

export default function ApiProxyView(props: ApiProxyViewProps) {
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
            services.config().saveApiProxyConfig(serverInfo).subscribe({
                next: () => enqueueSnackbar("Api proxy config saved!", {variant: 'success'}),
                error: (err) => enqueueSnackbar("Failed to save api proxy config!", {variant: 'error'})
            });
        }
    }, [services, serverInfo, enqueueSnackbar]);

    return <div className={'api-proxy'}>
        <div className={'api-proxy__toolbar'}><label>Api-Proxy</label>
            <button title={'Save'} onClick={handleSave}>Save</button>
        </div>
        <div className={'api-proxy__content'}>
            <div className={'api-proxy__content-table'}>
                {
                    SERVER_INFO_FIELDS.map(field =>
                        <div key={'api-proxy_field_' + field.name} className={'api-proxy__content-row'}>
                            <div className={'api-proxy__content-col  api-proxy__content-col-label'}>
                                <label>{field.caption}</label>
                            </div>
                            <div className={'api-proxy__content-col api-proxy__content-col-value'}>
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