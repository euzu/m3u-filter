import React, {ReactNode, useEffect, useRef, useState} from "react";

import './popup-menu.scss';
import ClickAwayListener from "../../utils/click-away-listener";

interface PopupMenuProps {
    position: { x: number, y: number },
    children: ReactNode,
    onHide: () => void;
}

export default function PopupMenu(props: PopupMenuProps) {
    const {position, onHide} = props;
    const popupRef = useRef();
    const [popupPosition, setPopupPosition] = useState<any>({top: 0, left: -2000})

    useEffect(() => {
        if (position && popupRef.current) {
            const ww = window.innerWidth;
            const {offsetWidth} = popupRef.current;
            const style: any =
                (position.x + offsetWidth > ww)
                    ? {right: 12, top: position.y}
                    : {left: position.x, top: position.y};
            setPopupPosition(style);
        }
    }, [position])

    return <>{props.position && <ClickAwayListener onClickAway={onHide}>
        <div className="popup-menu" ref={popupRef} style={popupPosition}>
            {props.children}
        </div>
    </ClickAwayListener>}</>
}