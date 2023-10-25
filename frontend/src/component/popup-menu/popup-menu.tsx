import React, {ReactChildren, ReactNode, useCallback, useEffect, useRef, useState} from "react";

import './popup-menu.scss';
import ClickAwayListener from "../../utils/click-away-listener";

interface PopupMenuProps {
    position: {x: number, y: number},
    children: ReactNode,
    onHide: () => void;
}

export default function PopupMenu(props: PopupMenuProps) {

    const popupRef = useRef();
    const [position, setPosition] = useState<any>({top:0, left: -2000})

    const handleClickAway = useCallback(() => {
        props.onHide?.();
    }, [props.onHide]);

    useEffect(() => {
        if (props.position &&  popupRef.current) {
            const ww = window.innerWidth;
            const {offsetWidth} = popupRef.current;
            const style: any =
                (props.position.x + offsetWidth > ww)
                    ? {right:12, top: props.position.y}
                    : {left:props.position.x, top: props.position.y};
            setPosition(style);
        }
    }, [props.position])

    return <>{props.position && <ClickAwayListener onClickAway={handleClickAway}>
        <div className="popup-menu" ref={popupRef} style={position}>
            {props.children}
        </div>
    </ClickAwayListener>}</>
}