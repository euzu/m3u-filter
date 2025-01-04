import React, {useCallback, useRef, KeyboardEvent, useState} from "react";
import './playlist-filter.scss';
import {getIconByName} from "../../icons/icons";
import InputField from "../input-field/input-field";

interface PlaylistFilterProps {
    onFilter: (filter: string, regexp: boolean) => void;
}

export default function PlaylistFilter(props: PlaylistFilterProps) {
    const {onFilter} = props;
    const textField = useRef<HTMLInputElement>(null);
    const [useRegexp, setUseRegexp] = useState<boolean>(false);

    const handleSearch = useCallback(() => {
        const value = textField.current.value;
        onFilter(value, useRegexp);
    }, [onFilter,useRegexp]);

    const handleKeyPress = useCallback((event: KeyboardEvent<any>) => {
        if (event.key === 'Enter') {
            handleSearch();
        }
    }, [handleSearch]);

    const handleRegexp = useCallback(() => {
        setUseRegexp(value => !value);
    }, []);

    return <div className={'playlist-filter'}>
        <InputField label={'Search'}>
            <input ref={textField} onKeyUp={handleKeyPress}/>
            <button title={'Regexp'} className={useRegexp ? 'playlist-filter__option-active' : ''} onClick={handleRegexp}>{getIconByName('Regexp')}</button>
            <button title={'Search'} onClick={handleSearch}>{getIconByName('Search')}</button>
        </InputField>
    </div>
}