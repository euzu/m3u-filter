import React, {useCallback, useEffect} from "react";
import "./date-picker.scss";
import ReactDatePicker from "react-datepicker";
import {getIconByName} from "../../icons/icons";
import useTranslator from "../../hook/use-translator";


interface DatePickerProps {
    name: string;
    value: any;
    onChange: (name: string, values: string[]) => void;
}

export default function DatePicker(props: DatePickerProps) {
    const {name, value, onChange} = props;
    const translate= useTranslator();
    const [selected, setSelected] = React.useState<any>(value);
    const datePickerRef = React.useRef<ReactDatePicker>(null);

    useEffect(() => {
        setSelected(value);
    }, [value]);

    const handleDateChange = useCallback((date: any) => {
        setSelected(date);
        datePickerRef.current.setOpen(false);
        onChange(name, date);
    }, [name, onChange]);

    const handleClick = useCallback(() => {
        if (!datePickerRef.current.isCalendarOpen()) {
            datePickerRef.current.setOpen(true);
        }
    }, []);

    const handleInfiniteDate = useCallback(() => {
        handleDateChange(new Date(0));
    }, [handleDateChange]);

    const handleNowDate = useCallback(() => {
        const date = new Date();
        date.setFullYear(date.getFullYear() + 1);
        handleDateChange(date);
    }, [handleDateChange]);

    return (
        <div className="date-picker-container">
            <div className="date-picker-container-wrapper" onClick={handleClick}>
                <ReactDatePicker
                    ref={datePickerRef}
                    showIcon={true}
                    icon={getIconByName('Calendar')}
                    dateFormat={'YYYY-MM-dd'}
                    // readOnly={true}
                    showMonthDropdown={true}
                    showYearDropdown={true}
                    minDate={new Date(0)}
                    onSelect={handleDateChange}
                    selected={selected}>
                </ReactDatePicker>
            </div>
            <div className={'date-picker-container__toolbar'}>
                <button title={translate('LABEL.INFINITE')} onClick={handleInfiniteDate}>{getIconByName('Unlimited')}</button>
                <button title={translate('LABEL.ONE_YEAR')} onClick={handleNowDate}>{getIconByName('Today')}</button>
            </div>
        </div>
    );
}
