import React, {JSX, useCallback, useState} from "react";
import './sidebar.scss';
import {getIconByName} from "../../icons/icons";
/* eslint-disable @typescript-eslint/no-empty-interface */
interface SidebarProps {
  children?: any;
}

export default function Sidebar(props: SidebarProps): JSX.Element {

    const {children} = props;
    const [collapsed, setCollapsed] = useState<boolean>(true);

    const handleToggle = useCallback(() => {
        setCollapsed(value => !value);
    }, []);

    return <div className={'sidebar'}>
        <div className={'sidebar-content' + (collapsed ? ' sidebar-collapsed' : ' sidebar-expanded')}>
            {children}
        </div>
        <div className={'sidebar-toggle'} onClick={handleToggle}>
            {getIconByName(collapsed ? 'ArrowDown' : 'ArrowUp')}
        </div>
    </div>;

}
