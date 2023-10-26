import React, {useCallback} from "react";
import './checkbox.scss';

interface CheckboxProps {
    label: string;
    value: any;
    onSelect: (checked: boolean, value: any) => void;
}

export default function Checkbox(props: CheckboxProps) {

    const {label, value, onSelect} = props;

    const handleSelect = useCallback((evt: any) => {
        onSelect?.(evt.target.checked, value);
    }, [value, onSelect]);

    return <label className="checkbox-container">
        {label}
        <input type="checkbox" onClick={handleSelect}/>
        <span className="checkmark"></span>
    </label>;
}