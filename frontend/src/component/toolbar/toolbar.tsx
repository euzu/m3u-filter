import React from "react";
import './toolbar.scss';
import useTranslator from "../../hook/use-translator";

interface ToolbarProps {
  onDownload: () => void;
}

export default function Toolbar(props: ToolbarProps) {
    const translate = useTranslator();
    return <div className={'toolbar'}>
        <button title={translate('LABEL.SAVE')} onClick={props.onDownload}>
            Save
        </button>
    </div>
}