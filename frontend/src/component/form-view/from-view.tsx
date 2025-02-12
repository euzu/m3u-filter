import React, {useCallback} from "react";
import './form-view.scss';
import Checkbox from "../checkbox/checkbox";
import TagSelect from "../tag-select/tags-select";
import MapEditor from "../map-editor/map-editor";
import TagInput from "../tag-input/tag-input";
import ScheduleEditor from "../schedule-editor/schedule-editor";
import DatePicker from "react-date-picker";
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
        if (data) {
            data[value] = checked;
        }
    }, [data]);

    const handleChange = useCallback((field: string, value: any) => {
        if (data) {
            data[field] = value;
        }
    }, [data]);

    const getFieldInput = useCallback((field: FormField) => {
        switch (field.fieldType) {
            case FormFieldType.READONLY:
                return <span>{data?.[field.name]}</span>;
            case FormFieldType.CHECK:
                return <Checkbox label={undefined} value={field.name} checked={data?.[field.name]} onSelect={handleCheckboxChange}></Checkbox>
            case FormFieldType.MULTI_SELECT:
                return <TagSelect options={field.options} name={field.name}
                                  defaultValues={data?.[field.name]} multi={true} onSelect={handleChange}></TagSelect>
            case FormFieldType.SINGLE_SELECT:
                return <TagSelect options={field.options} name={field.name}
                                  defaultValues={data?.[field.name]} multi={false} onSelect={handleChange}></TagSelect>
            case FormFieldType.MAP:
                return <div className="form-view__map-editor"><MapEditor onChange={handleChange} name={field.name} values={data?.[field.name]}></MapEditor></div>
            case FormFieldType.TAGS:
                return <TagInput placeHolder={''} onChange={handleChange} name={field.name} values={data?.[field.name] || []}></TagInput>
            case FormFieldType.SCHEDULE:
                return <ScheduleEditor onChange={handleChange} name={field.name} values={data?.[field.name] || []} sources={data?.sources || []}></ScheduleEditor>
            case FormFieldType.DATE:
                return <DatePicker onChange={handleValueChange} name={field.name} value={data?.[field.name] || []}></DatePicker>
            case FormFieldType.NUMBER:
            case FormFieldType.TEXT:
            default:
                return <input defaultValue={data?.[field.name]} data-field={field.name}
                              onChange={handleValueChange}></input>;
        }
    }, [data, handleChange, handleValueChange,handleCheckboxChange]);

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
