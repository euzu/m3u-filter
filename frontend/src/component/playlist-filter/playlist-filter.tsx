import React, {useMemo, useCallback, useRef, KeyboardEvent} from "react";
import './playlist-filter.scss';
import {getIconByName} from "../../icons/icons";

interface PlaylistFilterProps {
   onFilter: (filter: string) => void;
}

export default function PlaylistFilter(props: PlaylistFilterProps) {
    const {onFilter} = props;
    const textField = useRef<HTMLInputElement>();

    const handleSearch = useCallback(() => {
        const value = textField.current.value;
        onFilter(value);
    }, [onFilter]);

    const handleKeyPress = useCallback((event:  KeyboardEvent<any>) => {
        if (event.key === 'Enter') {
            handleSearch();
        }
    }, [handleSearch]);

    return <div className={'playlist-filter'}>
        <div className={'input-field'} onKeyUp={handleKeyPress}>
            <label>Search</label>
            <input ref={textField}/>
            <button onClick={handleSearch}>{getIconByName('Search')}</button>
        </div>
    </div>
}