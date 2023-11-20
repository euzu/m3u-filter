import React, {useCallback, useRef} from "react";
import './checkbox.scss';

interface CheckboxProps {
    label?: string;
    value: any;
    onSelect: (checked: boolean, value: any, evt?: any) => void;
}

export default function Checkbox(props: CheckboxProps) {

    const {label, value, onSelect} = props;
    const inputRef = useRef<HTMLInputElement>();

    const handleSelect = useCallback((evt: any) => {
        evt.preventDefault();
        evt.stopPropagation();
        inputRef.current.checked = !inputRef.current.checked;
        onSelect?.(inputRef.current.checked, value, evt);
    }, [value, onSelect]);

    if (label) {
        return <label className="checkbox-container" onClick={handleSelect}>
            {label}
            <input ref={inputRef}  type="checkbox"/>
            <span className="checkmark"></span>
        </label>;
    } else {
        return <div className="checkbox-container" onClick={handleSelect}>
            <input ref={inputRef} type="checkbox"/>
            <span className="checkmark"></span>
        </div>
    }
}