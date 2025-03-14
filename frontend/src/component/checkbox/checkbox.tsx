import React, {useCallback, useLayoutEffect, useRef} from "react";
import './checkbox.scss';

interface CheckboxProps {
    label?: string;
    value: any;
    checked: boolean;
    onSelect: (value: any, checked: boolean, evt?: any) => void;
}

export default function Checkbox(props: CheckboxProps) {

    const {label, value, onSelect, checked} = props;
    const inputRef = useRef<HTMLInputElement>(undefined);

    const handleSelect = useCallback((evt: any) => {
        evt.preventDefault();
        evt.stopPropagation();
        inputRef.current.checked = !inputRef.current.checked;
        onSelect?.(value, inputRef.current.checked, evt);
    }, [value, onSelect]);

    useLayoutEffect(() => {
       if (inputRef.current) {
           inputRef.current.checked = checked;
       }
    }, [checked]);

    if (label) {
        return <label className="checkbox-container" onClick={handleSelect}>
            {label}
            <input ref={inputRef}  type="checkbox" defaultChecked={checked}/>
            <span className="checkmark"></span>
        </label>;
    } else {
        return <div className="checkbox-container" onClick={handleSelect}>
            <input ref={inputRef} type="checkbox" defaultChecked={checked}/>
            <span className="checkmark"></span>
        </div>
    }
}