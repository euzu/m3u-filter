import React, {forwardRef, useCallback, useImperativeHandle, useMemo, useRef, useState} from "react";
import './user-editor.scss';
import {Credentials} from "../../model/server-config";
import useTranslator from "../../hook/use-translator";
import FormView, {FormFieldType} from "../form-view/from-view";

export const STATUS_OPTIONS = [
    {value: 'Active', label: 'Active'},
    {value: 'Expired', label: 'Expired'},
    {value: 'Banned', label: 'Banned'},
    {value: 'Trial', label: 'Trial'},
    {value: 'Disabled', label: 'Disabled'},
    {value: 'Pending', label: 'Pending'},
];

const COLUMNS = [
    {name: 'username', label: 'LABEL.USERNAME', fieldType: FormFieldType.TEXT},
    {name: 'password', label: 'LABEL.PASSWORD', fieldType: FormFieldType.TEXT},
    {name: 'token', label: 'LABEL.TOKEN', fieldType: FormFieldType.TEXT},
    {name: 'server', label: 'LABEL.SERVER', fieldType: FormFieldType.SINGLE_SELECT},
    {name: 'proxy', label: 'LABEL.PROXY', fieldType: FormFieldType.PROXY_TYPE},
    {name: 'max_connections', label: 'LABEL.MAX_CON', fieldType: FormFieldType.NUMBER},
    {name: 'status', label: 'LABEL.STATUS', fieldType: FormFieldType.SINGLE_SELECT, options: STATUS_OPTIONS},
    {name: 'exp_date', label: 'LABEL.EXP_DATE', fieldType: FormFieldType.DATE},
    {name: 'ui_enabled', label: 'LABEL.UI_ENABLED', fieldType: FormFieldType.CHECK},
    {name: 'comment', label: 'LABEL.NOTES', fieldType: FormFieldType.TEXT}
];

interface UserViewProps {
    serverOptions: { value: string, label: string }[];
    onSubmit: (user: Credentials, target: string) => boolean;
}

export interface IUserEditor {
    edit: (user: Credentials, target: string) => void;
    close: () => void;
}

const UserEditor = forwardRef<IUserEditor, UserViewProps>((props: UserViewProps, ref: any) => {
    const {serverOptions, onSubmit} = props;
    const dialogRef = useRef(null);
    const translate = useTranslator();
    const formFields = useMemo(() => COLUMNS
        .map((c) => {
            let options = c.options;
            if  (c.name === 'server') {
                options = serverOptions;
            }
            return ({...c, options});
        }), [serverOptions]);

    const [user, setUser] = useState(undefined);
    const [target, setTarget] = useState(undefined);

    const edit = useCallback((user: Credentials, target_name: string) => {
        setUser(user);
        setTarget(target_name);
        dialogRef.current?.showModal();
    }, []);

    const close = useCallback(() => {
        dialogRef.current?.close();
    }, []);

    const reference = useMemo(() => ({ edit, close }), [edit, close]);

    useImperativeHandle(ref, () => reference);

    const handleSubmit = useCallback(() => {
        if (onSubmit(user, target)) {
            dialogRef.current.close();
        }
    }, [onSubmit, user, target]);

    return <dialog ref={dialogRef}>
        <div className={'user-editor'}>
            <div className={'user-editor__content'}>
                <FormView data={user} fields={formFields}></FormView>
            </div>
            <div className={'user-editor__toolbar'}>
                <button data-tooltip='LABEL.CANCEL' onClick={() => dialogRef.current?.close()}>{translate('LABEL.CANCEL')}</button>
                <button data-tooltip='LABEL.OK' onClick={handleSubmit}>{translate('LABEL.OK')}</button>
            </div>
        </div>
    </dialog>;
});

export default UserEditor;