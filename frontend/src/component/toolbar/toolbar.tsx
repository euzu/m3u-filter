import React from "react";
import './toolbar.scss';
import {Button} from "@material-ui/core";

interface ToolbarProps {
  onDownload: () => void;
}

export default function Toolbar(props: ToolbarProps) {
    return <div className={'toolbar'}>
        <Button variant="contained" color="primary" onClick={props.onDownload}>
            Save
        </Button>
    </div>
}