import React from "react";
import './preferences.scss';
import ServerConfig from "../../model/server-config";
import UserView from "../user-view/user-view";
import TargetUpdate from "../target-update/target-update";

interface PreferencesProps {
    config: ServerConfig
}

export default function Preferences(props: PreferencesProps) {
    const {config} = props;
    return <div className={'preferences'}>
        <div className={'card'}><UserView config={config}></UserView></div>
        <div className={'card'}><TargetUpdate config={config}></TargetUpdate></div>
    </div>
}