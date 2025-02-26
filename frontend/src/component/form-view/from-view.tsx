import React, {useCallback, useEffect, useMemo, useRef, useState} from "react";
import './form-view.scss';
import Checkbox from "../checkbox/checkbox";
import TagSelect from "../tag-select/tags-select";
import MapEditor from "../map-editor/map-editor";
import TagInput from "../tag-input/tag-input";
import ScheduleEditor from "../schedule-editor/schedule-editor";
import DatePicker from "../date-picker/date-picker";
import {genUuid} from "../../utils/uuid";
import {getIconByName} from "../../icons/icons";
import useTranslator from "../../hook/use-translator";
// export const isNumber = (value: string): boolean => {
//     return !isNaN(value as any);
// }

export enum FormFieldType {
    READONLY= 'readonly',
    TEXT = 'text',
    NUMBER = 'number',
    MULTI_SELECT = 'multi_select',
    SINGLE_SELECT = 'single_select',
    CHECK = 'checkbox',
    TAGS = 'tags',
    MAP = 'map',
    SCHEDULE = 'schedule',
    DATE = 'date'
}

export type FormField = {
    name: string,
    label: string,
    hint?: string,
    validator?: (value: any) => boolean,
    options?: { value: string, label: string }[],
    fieldType: FormFieldType
};

interface FormViewProps {
    data: any;
    fields: FormField[]
}

export default function FormView(props: FormViewProps) {
    const {data, fields} = props;
    const uuid = useMemo(() => genUuid(), []);
    const translate = useTranslator();
    const inputIds= useRef([]);
    const [formData, setFormData] = useState(data || {});

    useEffect(() => {
       if (data) {
           setFormData(data);
           inputIds.current.forEach((id)=> {
              let elem: any = document.getElementById(id);
              if (elem) {
                  const field = elem.dataset.field;
                  if (field) {
                      elem.value = data[field]  ?? '';
                  }
              }
           });
       }
    }, [data])

    const handleInputValueChange = useCallback((evt: any) => {
        const field = evt.target.dataset.field;
        const value = evt.target.value;
        if (data) {
            data[field] = value;
        }
        setFormData((prevData: any) => ({...prevData, [field]: value ?? ''}));
    }, [data]);

    const handleChange = useCallback((field: string, value: any) => {
        if (data) {
            data[field] = value;
        }
        setFormData((prevData: any) => ({...prevData, [field]: value}));
    }, [data]);

    const getFieldInput = useCallback((field: FormField) => {
        switch (field.fieldType) {
            case FormFieldType.READONLY:
                return <span>{formData?.[field.name]}</span>;
            case FormFieldType.CHECK:
                return <Checkbox label={undefined} value={field.name} checked={formData?.[field.name]} onSelect={handleChange}></Checkbox>
            case FormFieldType.MULTI_SELECT:
                return <TagSelect options={field.options} name={field.name}
                                  defaultValues={formData?.[field.name]} multi={true} onSelect={handleChange}></TagSelect>
            case FormFieldType.SINGLE_SELECT:
                return <TagSelect options={field.options} name={field.name}
                                  defaultValues={formData?.[field.name]} multi={false} onSelect={handleChange}></TagSelect>
            case FormFieldType.MAP:
                return <div className="form-view__map-editor"><MapEditor onChange={handleChange} name={field.name} values={formData?.[field.name]}></MapEditor></div>
            case FormFieldType.TAGS:
                return <TagInput placeHolder={''} onChange={handleChange} name={field.name} values={formData?.[field.name] || []}></TagInput>
            case FormFieldType.SCHEDULE:
                return <ScheduleEditor onChange={handleChange} name={field.name} values={formData?.[field.name] || []} sources={data?.sources || []}></ScheduleEditor>
            case FormFieldType.DATE:
                return <DatePicker  name={field.name} onChange={handleChange} value={formData?.[field.name] || undefined}></DatePicker>
            case FormFieldType.NUMBER:
            case FormFieldType.TEXT:
            default: {
                const input_id = uuid + field.name;
                inputIds.current.push(input_id);
                return <input id={input_id} type={'text'} data-field={field.name} onChange={handleInputValueChange}></input>;
            }
        }
    }, [uuid, formData, data, inputIds, handleChange, handleInputValueChange]);

    return <div className={'form-view'}>
        <div className={'form-view__table'}>
            {
                fields.map(field =>
                    <div key={'form-view_field_' + field.name} className={'form-view__row'}>
                        <div className={'form-view__col  form-view__col-label'}>
                            <label>{translate(field.label)}</label>
                            {field.hint && <span className={'label-hint'} data-tooltip={translate(field.hint)}>{getIconByName('Help')}</span>}
                        </div>
                        <div className={'form-view__col form-view__col-value'}>
                            {getFieldInput(field)}
                        </div>
                    </div>
                )
            }
        </div>
    </div>
}
