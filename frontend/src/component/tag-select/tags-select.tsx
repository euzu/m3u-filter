import './tag-select.scss';
import {useCallback, useEffect, useMemo, useState} from "react";
import {noop} from "rxjs";

interface TagSelectProps {
    name: string;
    options: { value: any, label: string }[]
    defaultValues?: any[];
    onSelect: (name: string, values: any) => void;
    multi?: boolean;
}

export default function TagSelect(props: TagSelectProps) {
    const {name, multi, options, defaultValues, onSelect} = props;
    const [selected, setSelected] = useState<Record<number, boolean>>({});
    const uuid = useMemo(() => Date.now() + '-' + Math.floor(Math.random() * 99999), []);

    useEffect(() => {
        if (defaultValues && options) {
            const selections: Record<number, boolean> = {}
            for (let i = 0; i < options.length; i++) {
                selections[i] = defaultValues.includes(options[i].value);
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
            return {[idx]: !!!selections[idx]} as any;
        });
    }, [multi]);

    return <div className={'tag-select'}>
        {options.map((o, idx) =>
            <span key={uuid + '-' + idx}
                  className={'tag-select__tag' + (selected[idx] ? ' tag-select__tag-selected' : '')}
                  data-idx={idx} onClick={handleTagClick}>{o.label}</span>
        )}
    </div>
}