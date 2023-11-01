import React, {useCallback} from "react";

import './tab-set.scss';

export interface TabSetTab {
    label: string;
    key: string;
}

interface TabSetProps {
    tabs: TabSetTab[];
    active: string;
    onTabChange: (key: string) => void;
}

export default function TabSet(props: TabSetProps) {
    const {tabs, active, onTabChange} = props;

    const handleTabClick = useCallback((evt: any) => {
        const key = evt.target.dataset.key;
        onTabChange(key);
    }, [onTabChange]);

    return <ul className={'tab-set'}>
        {tabs?.map(tab =>
            <li className={'tab-set__tab' + (tab.key === active ? ' tab-set__tab-active' : '')} key={'tab_' + tab.key} data-key={tab.key} onClick={handleTabClick}>{tab.label}</li>)}
    </ul>

}