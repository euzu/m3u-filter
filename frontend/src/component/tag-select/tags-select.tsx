import './tag-select.scss';
import {useCallback, useEffect, useMemo, useState} from "react";
import {noop} from "rxjs";
import {genUuid} from "../../utils/uuid";

interface TagSelectProps {
    name: string;
    options: { value: any, label: string }[]
    defaultValues?: any[];
    onSelect: (name: string, values: any) => void;
    multi?: boolean;
    radio?: boolean;
}

export default function TagSelect(props: TagSelectProps) {
    const {name, multi, options, defaultValues, onSelect, radio} = props;
    const [selected, setSelected] = useState<Record<number, boolean>>({});
    const uuid = useMemo(() => genUuid(), []);

    useEffect(() => {
        if (defaultValues && options) {
            const valueIsArray = Array.isArray(defaultValues);
            const selections: Record<number, boolean> = {}
            for (let i = 0; i < options.length; i++) {
                const optionValue = options[i].value;
                selections[i] = valueIsArray ? defaultValues.includes(optionValue) : defaultValues == optionValue;
            }
            setSelected(selections);
        }
        return noop;
    }, [defaultValues, options]);

    useEffect(() => {
        const selections = Object.keys(selected).map((key: any) =>
            selected[key] ? options[key].value : undefined
        ).filter(Boolean);
        onSelect(name, multi ? selections : (selections.length ? selections[0] : undefined));
        return noop;
    }, [name, selected, multi, onSelect, options]);

    const handleTagClick = useCallback((evt: any) => {
        const idx = evt.target.dataset.idx;
        setSelected((selections) => {
            if (multi) {
                selections[idx] = !!!selections[idx];
                return {...selections};
            }
            if (radio) {
                if (!!selections[idx]) {
                    return selections;
                } else {
                    return {[idx]: !!!selections[idx]} as any;
                }
            }
            return {[idx]: !!!selections[idx]} as any;
        });
    }, [multi, radio]);

    return <div className={'tag-select'}>
        {options.map((o, idx) =>
            <span key={uuid + '-' + idx}
                  className={'tag-select__tag' + (selected[idx] ? ' tag-select__tag-selected' : '')}
                  data-idx={idx} onClick={handleTagClick}>{o.label}</span>
        )}
    </div>
}