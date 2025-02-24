import React, {KeyboardEvent, useCallback, useEffect, useRef, useState} from "react";
import './playlist-filter.scss';
import {getIconByName} from "../../icons/icons";
import InputField from "../input-field/input-field";
import useTranslator from "../../hook/use-translator";

function isValidRegExp(pattern: string): boolean {
    try {
        new RegExp(pattern);
        return true;
    } catch (e) {
        return false;
    }
}

interface PlaylistFilterProps {
    options?: { filter: string, regexp: boolean }
    onFilter: (filter: string, regexp: boolean) => void;
}

export default function PlaylistFilter(props: PlaylistFilterProps) {
    const {onFilter, options} = props;
    const translate = useTranslator();
    const textField = useRef<HTMLInputElement>(undefined);
    const [useRegexp, setUseRegexp] = useState<boolean>(false);
    const [errorMsg, setErrorMsg] = useState<string | undefined>(undefined);

    useEffect(()=> {
        setUseRegexp(options?.regexp ?? false);
        if (textField.current) {
            textField.current.value = options?.filter ?? '';
        }
    }, [options?.regexp, options?.filter]);

    const handleSearch = useCallback(() => {
        const value = textField.current.value;
        setErrorMsg(undefined);
        if (useRegexp && value?.trim() !== '') {
            if (isValidRegExp(value)) {
                onFilter(value, useRegexp);
            } else {
                setErrorMsg(translate('MESSAGES.INVALID_REGEXP'));
            }
        } else {
            onFilter(value, useRegexp);
        }
    }, [onFilter, useRegexp, translate]);

    const handleClear = useCallback(() => {
        textField.current.value = '';
        handleSearch();
    }, [handleSearch]);

    const handleKeyPress = useCallback((event: KeyboardEvent<any>) => {
        if (event.key === 'Enter') {
            handleSearch();
        }
    }, [handleSearch]);

    const handleRegexp = useCallback(() => {
        setUseRegexp(value => !value);
    }, []);

    return <div className={'playlist-filter'}>
        <InputField label={translate('LABEL.SEARCH')}>
            <input type="text" ref={textField} onKeyUp={handleKeyPress}/>
            <button title={translate('LABEL.REGEXP')} className={useRegexp ? 'playlist-filter__option-active' : ''}
                    onClick={handleRegexp}>{getIconByName('Regexp')}</button>
            <button title={translate('LABEL.CLEAR')} onClick={handleClear}>{getIconByName('ClearSearch')}</button>
            <button title={translate('LABEL.SEARCH')} onClick={handleSearch}>{getIconByName('Search')}</button>
        </InputField>
        {errorMsg && <div className="playlist-filter__error-message">{errorMsg}</div>}
    </div>
}