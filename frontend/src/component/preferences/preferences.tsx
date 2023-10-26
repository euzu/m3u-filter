import React from "react";
import './preferences.scss';
import ServerConfig from "../../model/server-config";
import UserView from "../user-view/user-view";
import TargetUpdate from "../target-update/target-update";
import ServerInfoView from "../server-info-view/server-info-view";

interface PreferencesProps {
    config: ServerConfig
}

export default function Preferences(props: PreferencesProps) {
    const {config} = props;
    return <div className={'preferences'}>

        <div className={'card'}><TargetUpdate config={config}></TargetUpdate></div>
        <div className={'vert-group'}>
            <div className={'card'}><ServerInfoView config={config}></ServerInfoView></div>
            <div className={'card'}><UserView config={config}></UserView></div>
        </div>
    </div>
}