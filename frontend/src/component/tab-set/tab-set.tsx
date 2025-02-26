import React, {useCallback, useMemo} from "react";

import './tab-set.scss';
import {genUuid} from "../../utils/uuid";
import useTranslator from "../../hook/use-translator";

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
    const uuid = useMemo(() => genUuid(), []);
    const translate = useTranslator();

    const handleTabClick = useCallback((evt: any) => {
        const key = evt.target.dataset.key;
        onTabChange(key);
    }, [onTabChange]);

    const handleTabPress = useCallback((evt: any) => {
        let key = (evt as any).key;
        if (key === 'Enter' || key === ' ') {
            handleTabClick(evt)
        }
        const rightArrow = key === 'ArrowRight';
        const leftArrow = key === 'ArrowLeft';
        if (rightArrow || leftArrow) {
            const key = evt.target.dataset.key;
            let idx = tabs?.findIndex( t => t.key === key);
            if (idx >= 0) {
                if (leftArrow) {
                    idx  -= 1;
                } else if (rightArrow) {
                    idx += 1;
                }
                if (idx > tabs.length-1) {
                    idx = 0;
                } else if (idx < 0) {
                    idx = tabs.length-1;
                }
            }
            let elem = document.getElementById(uuid + '-tab-' + idx)
            elem?.focus();
        }
    }, [uuid, tabs, handleTabClick]);

    return <ul className={'tab-set'}>
        {tabs?.map((tab, idx) =>
            <li tabIndex={0} className={'tab-set__tab' + (tab.key === active ? ' tab-set__tab-active' : '')}
                key={'tab_' + tab.key} data-key={tab.key} id={uuid + '-tab-' + idx}
                onKeyUp={handleTabPress}
                onClick={handleTabClick}>{translate(tab.label)}</li>)}
    </ul>

}