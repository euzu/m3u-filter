import {useServices} from "../../provider/service-provider";
import useTranslator from "../../hook/use-translator";
import {useEffect, useState} from "react";
import {ServerStatus} from "../../model/server-status";
import {interval} from "rxjs";
import {first} from "rxjs/operators";
import './status-view.scss';
import React from "react";

const REQUEST_INTERVAL = 5000;
const STATUS_COLUMNS = [
    "status",
    "version",
    "build_time",
    "server_time",
    "memory",
    "cache",
    "active_users",
    "active_user_connections",
    "active_provider_connections"];

export default function StatusView() {
    const services = useServices();
    const translate = useTranslator();
    const [status, setStatus] = useState<ServerStatus>();

    useEffect(() => {
        const sub = interval(REQUEST_INTERVAL).subscribe(() => services.status().getServerStatus().pipe(first()).subscribe((data: any) => setStatus(data)));
        return () => sub.unsubscribe();
    }, [services]);


    return <div className={'status-view'}>
        <div className={'status-view__content'}>
            {status && STATUS_COLUMNS.map(col => <React.Fragment key={'status.' + col} >
                    <div className={'status-view__col-label'}>{translate('LABEL.' + col.toUpperCase())}</div>
                    <div className={'status-view__col-value'}>{
                        (col === "active_provider_connections")  ? JSON.stringify((status as any)?.[col]) : (status as any)?.[col]
                    }</div>
                </React.Fragment>
            )}
        </div>
    </div>
}