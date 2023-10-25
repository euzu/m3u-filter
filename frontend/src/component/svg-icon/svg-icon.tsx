import './svg-icon.scss';
import * as React from "react";

interface SvgIconProps {
    name: string;
    path: string;
}

function SvgIcon(props: SvgIconProps): React.JSX.Element {
    return <svg className="svg-icon" focusable="false" aria-hidden="true" data-testid={props.name} height="100" width="100" viewBox='0 0 24 24'>
        <path d={props.path}/>
    </svg>;
}

SvgIcon.displayName = 'SvgIcon';
export default SvgIcon;
