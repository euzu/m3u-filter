import React from "react";
import './preferences.scss';
import ServerConfig from "../../model/server-config";
import UserView from "../user-view/user-view";

interface PreferencesProps {
    config: ServerConfig
}

export default function Preferences(props: PreferencesProps) {
    const {config} = props;
    return <div className={'preferences'}>
        <UserView config={config}></UserView>
    </div>
}