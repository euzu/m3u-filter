import React, {useCallback, useMemo} from "react";
import './api-proxy-view.scss';
import ServerConfig, {ServerInfo} from "../../model/server-config";
import {useSnackbar} from "notistack";
import {useServices} from "../../provider/service-provider";
import FormView, {FormFieldType} from "../form-view/from-view";

const isNumber = (value: string): boolean => {
    return !isNaN(value as any);
}

const SERVER_INFO_FIELDS = [
    {name: 'protocol', label: 'Protocol',  fieldType: FormFieldType.SINGLE_SELECT,
        options:[{value: 'http', label:'http'}, {value: 'https', label:'https'}]},
    {name: 'ip', label: 'IP',  fieldType: FormFieldType.TEXT},
    {name: 'http_port', label: 'HTTP port', fieldType: FormFieldType.NUMBER, validator: isNumber},
    {name: 'https_port', label: 'HTTPS port', fieldType: FormFieldType.NUMBER,validator: isNumber},
    {name: 'rtmp_port', label: 'RTMP port', fieldType: FormFieldType.NUMBER,validator: isNumber},
    {name: 'timezone', label: 'Timezone', fieldType: FormFieldType.TEXT},
    {name: 'message', label: 'Message', fieldType: FormFieldType.TEXT},
];

interface ApiProxyViewProps {
    config: ServerConfig;
}

export default function ApiProxyView(props: ApiProxyViewProps) {
    const {config} = props;
    const services = useServices();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const serverInfo = useMemo<ServerInfo>(() => config?.api_proxy?.server, [config]);

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
            <FormView data={serverInfo} fields={SERVER_INFO_FIELDS}></FormView>
        </div>
    </div>
}