import React, {ReactNode} from "react";
import './panel.scss';

interface PanelProps {
    value: any;
    active: any;
    children: ReactNode;
}

export default function Panel(props: PanelProps) {
    const {value, active, children} = props;
    return <div className={'panel' + (value === active ? '' : ' hidden')}>{children}</div>
}