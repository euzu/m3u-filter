import React, {useCallback, useMemo} from "react";
import './api-proxy-view.scss';
import ServerConfig, {ApiProxyServerInfo} from "../../model/server-config";
import {useSnackbar} from "notistack";
import {useServices} from "../../provider/service-provider";
import FormView, {FormFieldType} from "../form-view/from-view";
import {getIconByName} from "../../icons/icons";

const isNumber = (value: string): boolean => {
    return !isNaN(value as any);
}

const SERVER_INFO_FIELDS = [
    {name: 'name', label: 'Name',  fieldType: FormFieldType.READONLY},
    {name: 'protocol', label: 'Protocol',  fieldType: FormFieldType.SINGLE_SELECT,
        options:[{value: 'http', label:'http'}, {value: 'https', label:'https'}]},
    {name: 'host', label: 'Host',  fieldType: FormFieldType.TEXT},
    {name: 'port', label: 'Port', fieldType: FormFieldType.NUMBER, validator: isNumber},
    {name: 'timezone', label: 'Timezone', fieldType: FormFieldType.TEXT},
    {name: 'message', label: 'Message', fieldType: FormFieldType.TEXT},
    {name: 'path', label: 'Path', fieldType: FormFieldType.TEXT},
];

interface ApiProxyViewProps {
    config: ServerConfig;
}

export default function ApiProxyView(props: ApiProxyViewProps) {
    const {config} = props;
    const services = useServices();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const serverInfo = useMemo<ApiProxyServerInfo[]>(() => config?.api_proxy?.server, [config]);

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
            <div className={'api-proxy__content-area'}>
            {serverInfo?.map((server, idx) => <div className={"card"} key={server.name + idx}>
                    <FormView data={server} fields={SERVER_INFO_FIELDS}></FormView>
                </div>)
            }
            </div>
        </div>
        <div className="api-proxy__content-help">
            <span className="api-proxy__content-help-warn-icon">{getIconByName('Warn')}</span>
            <span>You need to restart to apply changes.</span>
        </div>
    </div>
}