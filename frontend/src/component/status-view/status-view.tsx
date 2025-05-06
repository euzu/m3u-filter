import {useServices} from "../../provider/service-provider";
import useTranslator from "../../hook/use-translator";
import React, {useCallback, useEffect, useState} from "react";
import {ServerIpCheck, ServerStatus} from "../../model/server-status";
import {interval} from "rxjs";
import {first} from "rxjs/operators";
import './status-view.scss';

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
    const [ipCheck, setIpCheck] = useState<any>();

    const checkIp = useCallback(() => {
        services.status().getIpCheck().pipe(first()).subscribe({
            next: (data: ServerIpCheck) => setIpCheck(data),
            error: (err) => {
                setIpCheck(undefined);
            },
        });
    }, [services])

    useEffect(() => {
        const fetchData = () => services.status().getServerStatus().pipe(first()).subscribe({
            next: (data: any) => setStatus(data),
            error: () => {
            }
        });
        fetchData();
        const sub = interval(REQUEST_INTERVAL).subscribe(fetchData);
        checkIp();
        return () => sub.unsubscribe();
    }, [services, checkIp])


    return <div className={'status-view'}>
        {ipCheck && <>
            <div className={'status-view__section-title'}>{translate("LABEL.IP")}</div>
            <div className={'status-view__content'}>
                <div className={"status-view__col-label"}>{translate("LABEL.IPv4")}</div>
                <div className={'status-view__col-value'}>{ipCheck?.ipv4 ?? '?'}</div>
                <div className={"status-view__col-label"}>{translate("LABEL.IPv6")}</div>
                <div className={'status-view__col-value'}>{ipCheck?.ipv6 ?? '?'}</div>
            </div>
        </>}
        <div className={'status-view__section-title'}>{translate("LABEL.STATUS")}</div>
        <div className={'status-view__content'}>
            {status && STATUS_COLUMNS.map(col => <React.Fragment key={'status.' + col}>
                    <div className={'status-view__col-label'}>{translate('LABEL.' + col.toUpperCase())}</div>
                    <div className={'status-view__col-value'}>{
                        (col === "active_provider_connections") ? JSON.stringify((status as any)?.[col]) : (status as any)?.[col]
                    }</div>
                </React.Fragment>
            )}
        </div>
    </div>
}