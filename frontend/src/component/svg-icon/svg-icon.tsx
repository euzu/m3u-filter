import './svg-icon.scss';
import * as React from "react";

interface SvgIconProps {
    name: string;
    path: string;
}

function SvgIcon(props: SvgIconProps): React.JSX.Element {
    return <svg className="svg-icon" focusable="false" aria-hidden="true" data-testid={props.name} viewBox='0 0 24 24'>
        <path d={props.path}/>
    </svg>;
}

SvgIcon.displayName = 'SvgIcon';
export default SvgIcon;
