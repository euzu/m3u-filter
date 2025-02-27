import React, {forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState} from "react";
import './source-selector-editor.scss';
import useTranslator from "../../hook/use-translator";
import FormView, {FormFieldType} from "../form-view/from-view";
import ServerConfig from "../../model/server-config";
import {noop} from "rxjs";
import TagSelect from "../tag-select/tags-select";
import TabSet from "../tab-set/tab-set";
import {enqueueSnackbar} from "notistack";
import {PlaylistRequest, PlaylistRequestType} from "../../model/playlist-request";

const SOURCE_SELF_HOSTED = "self_hosted"
const SOURCE_PROVIDER = "provider"
const SOURCE_CUSTOM = "custom"

const SOURCE_TYPE_XC = "xtream";
const SOURCE_TYPE_M3U = "m3u";

const SOURCE_TABS = [
    {label: "LABEL.SELF_HOSTED", key: SOURCE_SELF_HOSTED},
    {label: "LABEL.PROVIDER", key: SOURCE_PROVIDER},
    {label: "LABEL.CUSTOM", key: SOURCE_CUSTOM},
];

const SOURCE_TYPES = [
    {label: "LABEL.XTREAM_CODES", value: SOURCE_TYPE_XC},
    {label: "LABEL.M3U", value: SOURCE_TYPE_M3U},
];

const XC_COLUMNS = [
    {name: 'username', label: 'LABEL.USERNAME', fieldType: FormFieldType.TEXT},
    {name: 'password', label: 'LABEL.PASSWORD', fieldType: FormFieldType.TEXT},
    {name: 'url', label: 'LABEL.HOST', fieldType: FormFieldType.TEXT},
];

const M3U_COLUMNS = [
    {name: 'url', label: 'LABEL.URL', fieldType: FormFieldType.TEXT},
];

interface SourceSelectorProps {
    serverConfig: ServerConfig;
    onSubmit: (req: PlaylistRequest) => boolean;
}

export interface ISourceSelectorEditor {
    open: () => void;
    close: () => void;
}

const SourceSelectorEditor = forwardRef<ISourceSelectorEditor, SourceSelectorProps>((props: SourceSelectorProps, ref: any) => {
    const {serverConfig, onSubmit} = props;
    const dialogRef = useRef(null);
    const dataRef = useRef<any>({});
    const [inputs, setInputs] = useState([]);
    const [targets, setTargets] = useState([]);
    const [activeTab, setActiveTab] = useState<string>(SOURCE_SELF_HOSTED);
    const [sourceType, setSourceType] = useState<string>(SOURCE_TYPE_XC);
    const sourceRef = useRef({input: undefined, target: undefined});
    const translate = useTranslator();

    const open = useCallback(() => {
        dialogRef.current?.showModal();
    }, []);

    const close = useCallback(() => {
        dialogRef.current?.close();
    }, []);

    const reference = useMemo(() => ({open, close}), [open, close]);

    useImperativeHandle(ref, () => reference);

    const handleSubmit = useCallback(() => {
        let result = undefined;
        if (activeTab === SOURCE_SELF_HOSTED) {
            // eslint-disable-next-line eqeqeq
            if (sourceRef.current.target != null) {
                result = {rtype: PlaylistRequestType.TARGET, sourceId: sourceRef.current.target, sourceName: targets.find(i => i.id === sourceRef.current.target)?.name};
            }
        } else if (activeTab === SOURCE_PROVIDER) {
            // eslint-disable-next-line eqeqeq
            if (sourceRef.current.input != null) {
                result = {rtype: PlaylistRequestType.INPUT, sourceId: sourceRef.current.input, sourceName: inputs.find(i => i.id === sourceRef.current.input)?.name};
            }
        } else {
            let record : any = dataRef.current;
            if (sourceType === SOURCE_TYPE_XC) {
                let username = record.username?.trim()
                let password = record.password?.trim()
                let url = record.url?.trim()
                if (username && password && url) {
                    result = {rtype: PlaylistRequestType.XTREAM, username, password, url};
                }
            } else {
                let url = record.url?.trim()
                if (url) {
                    result = {rtype: PlaylistRequestType.M3U, url};
                }
            }
        }
        if (!result) {
            enqueueSnackbar(translate('MESSAGES.SOURCE_SELECTOR.MISSING_SELECTION'), {variant: 'error'});
        } else {
            if (onSubmit(result)) {
                dialogRef.current.close();
            }
        }
    }, [onSubmit, activeTab, inputs, targets, sourceType, translate]);

    useEffect(() => {
        if (serverConfig) {
            setInputs(serverConfig.sources?.flatMap(source => source.inputs) ?? []);
            setTargets(serverConfig.sources?.flatMap(source => source.targets) ?? []);
        }
        return noop;
    }, [serverConfig]);


    const handleSourceSelect = useCallback((field: string, value: any) => {
        setTimeout(() => {
            if (field === 'source_type') {
                setSourceType(value)
            } else {
                sourceRef.current = {...sourceRef.current, [field]: value};
            }
        }, 0);
    }, []);

    return <dialog ref={dialogRef}>
        <div className={'source-selector-editor'}>
            <div className={'source-selector-editor__content'}>
                <TabSet tabs={SOURCE_TABS} active={activeTab} onTabChange={setActiveTab}></TabSet>
                <div
                    className={'source-selector-editor__content-tags' + (activeTab !== SOURCE_TABS[0].key ? ' hidden' : '')}>
                    <TagSelect name={'target'} onSelect={handleSourceSelect}
                               options={targets.map(target => ({value: target.id, label: target.name}))}></TagSelect>
                </div>
                <div
                    className={'source-selector-editor__content-tags' + (activeTab !== SOURCE_TABS[1].key ? ' hidden' : '')}>
                    <TagSelect name={'input'} onSelect={handleSourceSelect}
                               options={inputs.map(input => ({value: input.id, label: input.name}))}></TagSelect>
                </div>
                <div
                    className={'source-selector-editor__content-form' + (activeTab !== SOURCE_TABS[2].key ? ' hidden' : '')}>
                    <TagSelect name={'source_type'} onSelect={handleSourceSelect} options={SOURCE_TYPES} defaultValues={[sourceType]}></TagSelect>
                    <FormView data={dataRef.current} fields={sourceType === 'xtream' ? XC_COLUMNS : M3U_COLUMNS}></FormView>
                </div>
            </div>
            <div className={'source-selector-editor__toolbar'}>
                <button data-tooltip='LABEL.CANCEL'
                        onClick={() => dialogRef.current?.close()}>{translate('LABEL.CANCEL')}</button>
                <button data-tooltip='LABEL.OK' onClick={handleSubmit}>{translate('LABEL.OK')}</button>
            </div>
        </div>
    </dialog>;
});

export default SourceSelectorEditor;