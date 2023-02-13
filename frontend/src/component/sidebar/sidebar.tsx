import React, {useCallback, useState} from "react";
import './sidebar.scss';
import {ArrowLeft, ArrowRight} from "@mui/icons-material";
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
            {collapsed ? <ArrowRight/> : <ArrowLeft/>}
        </div>
    </div>;

}
