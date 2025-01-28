import './map-editor.scss';

import React, {useCallback, useEffect, useMemo, useRef, useState} from "react";
import {genUuid} from "../../utils/uuid";
import {getIconByName} from "../../icons/icons";

interface KeyValue {
    key: string;
    value: string;
}

const mapToList = (data: Record<string, string>) => data ? Object.keys(data).map(key => ({key, value: data[key]})) : [];
const listToMap = (data: KeyValue[]) => data?.reduce((acc: any, curVal) => {acc[curVal.key] = curVal.value; return acc;}, {});
const containsKey = (key: string, data: KeyValue[]) => data?.find((keyValue: KeyValue) => keyValue.key === key);

interface MapEditorProps {
    name: string;
    values: Record<string, string>;
    onChange: (name: string, values: Record<string, string>) => void;
}

export default function MapEditor(props: MapEditorProps) {
    const {name, values, onChange} = props;
    const uuid = useMemo(() => genUuid(), []);
    const keyRef = useRef<HTMLInputElement>(undefined);
    const valRef = useRef<HTMLInputElement>(undefined);
    const [data, setData] = useState<KeyValue[]>([]);

    useEffect(() => {
        if (values) {
            setData(mapToList(values));
        }
    }, [values]);

    useEffect(() => {
        onChange(name, listToMap(data));
    }, [data, name, onChange]);

    const handleHeaderRemove = useCallback((event: any) => {
        const key = event.target.dataset.key;
        if (key) {
            setData(data => data?.filter((keyValue: KeyValue) => keyValue.key !== key));
        }

    }, []);

    const handleAddKeyValue = useCallback((event: any) => {
        let key = keyRef.current?.value?.trim();
        let value = valRef.current?.value?.trim();
        if (key.length> 0 && value.length > 0) {
            setData(data => {
                if (!containsKey(key, data)) {
                    data.push({key, value});
                    return [...data];
                }
                return data;
            });
        }
    }, []);

    return <div className={'map-editor'}>
        <div className={'map-editor__input'}>
            <div className='map-editor__input-key'><input ref={keyRef}></input></div>
            <div className='map-editor__input-value'><input ref={valRef}></input></div>
            <div className='map-editor__input-toolbar'><button onClick={handleAddKeyValue}>Add</button></div>
        </div>
        <div className={'map-editor__table'}>
            {data?.map((keyValue: KeyValue, idx) =>
                <div key={uuid + '-' + keyValue.key + idx} className={'map-editor__row'}>
                    <div className={'map-editor__col map-editor__col-key'}>{keyValue.key}</div>
                    <div className={'map-editor__col map-editor__col-value'}>{keyValue.value}</div>
                    <div className={'map-editor__col map-editor__toolbar'}>
                           <span data-key={keyValue.key} onClick={handleHeaderRemove}>
                               {getIconByName('Delete')}
                           </span>
                    </div>
                </div>
            )}
        </div>
    </div>
}