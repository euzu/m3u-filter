import './proxy-select.scss';
import {useEffect, useState} from "react";
import {noop} from "rxjs";

const getSubsStr = (subs: any): string  => {
    let result = Object.keys(subs).filter(key => subs[key]);
    if (result.length > 0 && result.length < 3) {
        return '[' + result.join(',') + ']';
    }
    return '';
};

const getSubs = (value: string) => {
    if (value?.startsWith('reverse')) {
        let result = {live:true, vod:true, series: true};
        if (!value.includes('live')) {
            result.live = false;
        }
        if (!value.includes('vod')) {
            result.vod = false;
        }
        if (!value.includes('series')) {
            result.series = false;
        }
        return result;
    }
    return {live: false, vod:false, series: false};
}

interface ProxySelectProps {
    name: string;
    value: string;
    onSelect: (name: string, values: any) => void;
}

export default function ProxySelect(props: ProxySelectProps) {
    const {name, value, onSelect} = props;
    const [proxyType, setProxyType] = useState(() => value == undefined ? 0 : (value.startsWith('redirect')) ? 2 : 1);
    const [subs, setSubs] = useState(() => getSubs(value));

    useEffect(() => {
         if (value) {
             setProxyType(value.startsWith('redirect') ? 2 : 1);
             setSubs(getSubs(value));
         }
         return noop;
    }, [value]);

    const changeProxyType = (pt: number) => {
        setProxyType(pt);
        onSelect(name, pt === 1 ? ('reverse' + getSubsStr(subs))  : 'redirect');
    };

    const toggle = (sub: string)=> {
        setSubs((subs: any) => {
           subs[sub] = !subs[sub];
           return {...subs};
        });
        onSelect(name, proxyType === 1 ? ('reverse' + getSubsStr(subs))  : 'redirect');
    };

    return <div className={'proxy-select'}>
        <div className={'proxy-select__tag' + (proxyType===1 ? ' proxy-select__tag-selected' :'')} onClick={() => changeProxyType(1)}>
            <div>Reverse</div>
            <div>
            <span className={subs.live ? 'selected' : ''} onClick={() => toggle('live')}>Live</span>
            <span className={subs.vod ? 'selected' : ''} onClick={() => toggle('vod')}>Vod</span>
            <span className={subs.series ? 'selected' : ''} onClick={() => toggle('series')}>Series</span>
            </div>
        </div>
        <div className={'proxy-select__tag' + (proxyType===2 ? ' proxy-select__tag-selected' :'')} onClick={() => changeProxyType(2)}>
            Redirect
        </div>
    </div>
}