import React, {useCallback} from "react";
import "./date-picker.scss";
import ReactDatePicker from "react-datepicker";
import {getIconByName} from "../../icons/icons";


interface DatePickerProps {
    name: string;
    value: any;
    onChange: (name: string, values: string[]) => void;
}

export default function DatePicker(props: DatePickerProps) {
    const {name, value, onChange} = props;
    const [selected, setSelected] = React.useState<any>(value);
    const datePickerRef = React.useRef<ReactDatePicker>(null);

    const handleDateChange = useCallback((date: any) => {
        setSelected(date);
        datePickerRef.current.setOpen(false);
        onChange(name, date);
    }, [name, onChange]);

    const handleClick = useCallback(() => {
        console.log(datePickerRef.current.isCalendarOpen())
        if (!datePickerRef.current.isCalendarOpen()) {
            datePickerRef.current.setOpen(true);
        }
    }, []);


    return (
        <div className="date-picker-container" onClick={handleClick}>
            <ReactDatePicker
                ref={datePickerRef}
                showIcon={true}
                icon={getIconByName('Calendar')}
                dateFormat={'YYYY-MM-dd'}
                // readOnly={true}
                showMonthDropdown={true}
                showYearDropdown={true}
                onSelect={handleDateChange}
                selected={selected}>
            </ReactDatePicker>
        </div>
    );
}
