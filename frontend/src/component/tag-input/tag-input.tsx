import React, {useCallback, useEffect, useState} from "react";
import './tag-input.scss';
import TagsInput from "react-tagsinput";

interface TagInputProps {
    name: string;
    values: string[];
    onChange: (name: string, values: string[]) => void;
}

export default function TagInput(props: TagInputProps) {

    const {name, values, onChange} = props;

    const [tags, setTags] = useState( []);

    useEffect(() =>{
        if (values) {
            setTags(values);
        }
    }, [values]);

    const handleChange = useCallback((tags: any) => {
        setTags(tags)
        onChange(name, tags);
    }, [name, onChange]);

    return  <TagsInput inputProps={({placeholder: "Add Chat-Id"})} value={tags} onChange={handleChange}></TagsInput>
}