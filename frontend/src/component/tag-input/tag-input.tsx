import React from "react";
import { TagsInput } from "react-tag-input-component";
import "./tag-input.scss";

interface TagInputProps {
  name: string;
  values: string[];
  onChange: (name: string, values: string[]) => void;
}

export default function TagInput(props: TagInputProps) {
  const { name, values, onChange } = props;

  const handleTagsChange = (newTags: string[]) => {
    onChange(name, newTags);
  };

  return (
    <div className="tag-input-container">
      <TagsInput
        value={values}
        onChange={handleTagsChange}
        name={name}
        placeHolder="Add tags..."
      />
    </div>
  );
}
