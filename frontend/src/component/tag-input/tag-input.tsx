import React from "react";
import { TagsInput } from "react-tag-input-component";
import "./tag-input.scss";

interface TagInputProps {
  name: string;
  values: string[];
  onChange: (name: string, values: string[]) => void;
  placeHolder?: string;
}

export default function TagInput(props: TagInputProps) {
  const { name, values, onChange, placeHolder } = props;

  const handleTagsChange = (newTags: string[]) => {
    onChange(name, newTags);
  };

  return (
    <div className="tag-input-container">
      <TagsInput
        value={values}
        onChange={handleTagsChange}
        name={name}
        placeHolder={placeHolder ?? "Add tags..."}
      />
    </div>
  );
}
