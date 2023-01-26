import React, {useMemo, useCallback, useRef, KeyboardEvent} from "react";
import './playlist-filter.scss';
import {TextField, InputAdornment, IconButton} from "@mui/material";
import {Search} from "@mui/icons-material";

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

    const inputProps = useMemo(() => (
    {
        endAdornment: (
            <InputAdornment position="end">
                <IconButton
                    className={"icon-button"}
                    aria-label="select"
                    onClick={handleSearch}
                    edge="end">
                    <Search />
                </IconButton>
            </InputAdornment>
        )
    }
    ), [handleSearch]);

    return <div className={'playlist-filter'}>
        <TextField
            className={'text-input'}
            inputRef={textField} label="Search" variant="outlined"
            InputProps={inputProps}
            InputLabelProps={{
                shrink: true,
            }}
            onKeyUp={handleKeyPress}
        />
    </div>
}