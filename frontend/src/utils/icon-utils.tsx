import * as React from "react";
import SvgIcon from "../component/svg-icon/svg-icon";

export default function createIcon(name: string, path: string): React.JSX.Element {
    return <SvgIcon name={name} path={path}/>;
} 
