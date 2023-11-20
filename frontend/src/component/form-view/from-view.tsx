import React, {useCallback} from "react";
import './form-view.scss';
import Checkbox from "../checkbox/checkbox";
import TagSelect from "../tag-select/tags-select";
import MapEditor from "../map-editor/map-editor";

export const isNumber = (value: string): boolean => {
    return !isNaN(value as any);
}

export enum FormFieldType {
    TEXT = 'text',
    NUMBER = 'number',
    MULTI_SELECT = 'multi_select',
    SINGLE_SELECT = 'single_select',
    CHECK = 'checkbox',
    TAGS = 'tags',
    MAP = 'map'
}

export type FormField = {
    name: string,
    label: string,
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

    const handleValueChange = useCallback((evt: any) => {
        const field = evt.target.dataset.field;
        if (data) {
            data[field] = evt.target.value;
        }
    }, [data]);

    const handleCheckboxChange = useCallback((checked: boolean, value: any, evt?: any) => {
        const field = evt.target.dataset.field;
        if (data) {
            data[field] = checked;
        }
    }, [data]);

    const handleChange = useCallback((field: string, value: any) => {
        if (data) {
            data[field] = value;
        }
    }, [data]);

    const getFieldInput = useCallback((field: FormField) => {
        switch (field.fieldType) {
            case FormFieldType.CHECK:
                return <Checkbox label={undefined} value={field.name} onSelect={handleCheckboxChange}></Checkbox>
            case FormFieldType.MULTI_SELECT:
                return <TagSelect options={field.options} name={field.name}
                                  defaultValues={data?.[field.name]} multi={true} onSelect={handleChange}></TagSelect>
            case FormFieldType.SINGLE_SELECT:
                return <TagSelect options={field.options} name={field.name}
                                  defaultValues={data?.[field.name]} multi={false} onSelect={handleChange}></TagSelect>
            case FormFieldType.MAP:
                return <MapEditor onChange={handleChange} name={field.name} values={data?.[field.name]}></MapEditor>
            case FormFieldType.NUMBER:
            case FormFieldType.TEXT:
            default:
                return <input defaultValue={data?.[field.name]} data-field={field.name}
                              onChange={handleValueChange}></input>;
        }
    }, [data, handleValueChange,handleCheckboxChange]);

    return <div className={'form-view'}>
        <div className={'form-view__table'}>
            {
                fields.map(field =>
                    <div key={'form-view_field_' + field.name} className={'form-view__row'}>
                        <div className={'form-view__col  form-view__col-label'}>
                            <label>{field.label}</label>
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