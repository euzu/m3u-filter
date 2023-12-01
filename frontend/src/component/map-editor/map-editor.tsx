import './map-editor.scss';

import {KeyboardEvent, useCallback, useEffect, useMemo, useState} from "react";

interface KeyValue {
    key: string;
    value: string;
}

const mapToList = (data: Record<string, string>) => data ? Object.keys(data).map(key => ({key, value: data[key]})) : [];

interface MapEditorProps {
    name: string;
    values: Record<string, string>;
    onChange: (name: string, values: Record<string, string>) => void;
}

export default function MapEditor(props: MapEditorProps) {
    const {name, values, onChange} = props;
    const uuid = useMemo(() => Date.now() + '-' + Math.floor(Math.random() * 99999), []);
    const [data, setData] = useState<KeyValue[]>(mapToList(values) || []);

    useEffect(() => {
        onChange(name, data as any);
    }, [data, name, onChange]);

    const handleValueChange = useCallback((target: any) => {
        const is_key = target.dataset.field === 'key';
        const idx = target.dataset.idx;
        setData((values) => {
            if (is_key) {
                values[idx].key = target.value;
            } else {
                values[idx].value = target.value;
            }
            return [...values];
        });
    }, []);

    const handleKeyPress = useCallback((event: KeyboardEvent<any>) => {
        if (event.key === 'Enter') {
            handleValueChange(event.target as any);
        }
    }, [handleValueChange]);

    const handleBlur = useCallback((event: any) => {
        handleValueChange(event.target as any);
    }, [handleValueChange]);

    return <div className={'map-editor'}>
        <div className={'map-editor__table'}>
            {Object.keys(data)?.map((key: any, idx) =>
                <div key={uuid + '-' + idx} className={'map-editor__row'}>
                    <div className={'map-editor__col map-editor__col-key'}>
                        <input data-idx={idx} data-field={'key'} defaultValue={data?.[idx].key} onKeyUp={handleKeyPress}
                               onBlur={handleBlur}></input>
                    </div>
                    <div className={'map-editor__col map-editor__col-value'}>
                        <input data-idx={idx} data-field={'value'} defaultValue={data?.[idx].value as any}
                               onKeyUp={handleKeyPress} onBlur={handleBlur}></input>
                    </div>
                </div>
            )}
        </div>
    </div>
}