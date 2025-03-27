import './tag-select.scss';
import {useCallback, useEffect, useMemo, useState} from "react";
import {noop} from "rxjs";
import {genUuid} from "../../utils/uuid";
import useTranslator from "../../hook/use-translator";

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
    const translate = useTranslator();
    const [selected, setSelected] = useState<Record<number, boolean>>({});
    const uuid = useMemo(() => genUuid(), []);

    useEffect(() => {
        if (defaultValues && options) {
            const valueIsArray = Array.isArray(defaultValues);
            const selections: Record<number, boolean> = {}
            for (let i = 0; i < options.length; i++) {
                const optionValue = options[i].value;
                // eslint-disable-next-line eqeqeq
                selections[i] = valueIsArray ? defaultValues.includes(optionValue) : defaultValues == optionValue;
            }
            setSelected(selections);
        }
        return noop;
    }, [defaultValues, options]);

    const handleTagClick = useCallback((evt: any) => {
        const idx = evt.target.dataset.idx;
        setSelected((selections) => {
            const getSelections = () => {
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
            };
            const result = getSelections();
            const newSelections = Object.keys(result).map((key: any) => result[key] ? options[key].value : undefined).filter(Boolean);
            onSelect(name, multi ? newSelections : (newSelections.length ? newSelections[0] : undefined));
            return result;
        });
    }, [multi, radio, onSelect, name, options]);

    return <div className={'tag-select'}>
        {options.map((o, idx) =>
            <span key={uuid + '-' + idx}
                  className={'tag-select__tag' + (selected[idx] ? ' tag-select__tag-selected' : '')}
                  data-idx={idx} onClick={handleTagClick}>{translate(o.label)}</span>
        )}
    </div>
}