import React from "react";
import './toolbar.scss';

interface ToolbarProps {
  onDownload: () => void;
}

export default function Toolbar(props: ToolbarProps) {
    return <div className={'toolbar'}>
        <button onClick={props.onDownload}>
            Save
        </button>
    </div>
}