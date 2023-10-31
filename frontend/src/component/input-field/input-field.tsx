import React, {ReactNode} from "react";
import './input-field.scss';

interface InputFieldProps {
    label: string;
    children: ReactNode;
}

export default function InputField(props: InputFieldProps) {
    const {label, children} = props;
    return <div className={'input-field'}>
        <label>{label}</label>
        <div className={'input-field__content'}>
            {children}
        </div>
    </div>;

}