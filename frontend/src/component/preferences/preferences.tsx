import React, {useCallback, useState} from "react";
import './preferences.scss';
import ServerConfig from "../../model/server-config";
import UserView from "../user-view/user-view";
import TargetUpdateView from "../target-update-view/target-update-view";
import ApiProxyInfoView from "../api-proxy-view/api-proxy-view";
import {getIconByName} from "../../icons/icons";
import Panel from "../panel/panel";
import MainConfigView from "../main-config-view/main-config-view";

enum SidebarAction {
    Update = 'update',
    User = 'user',
    ApiServer = 'api_server',
    MainConfig = 'main_config'
}

const SIDEBAR_ACTIONS: { action: SidebarAction, icon: string }[] = [
    {action: SidebarAction.Update, icon: 'Refresh'},
    {action: SidebarAction.User, icon: 'User'},
    {action: SidebarAction.ApiServer, icon: 'ApiServer'},
    {action: SidebarAction.MainConfig, icon: 'Config'},
];

interface PreferencesProps {
    config: ServerConfig
}

export default function Preferences(props: PreferencesProps) {
    const {config} = props;
    const [activePage, setActivePage] = useState(SidebarAction.Update);

    const handleSidebarAction = useCallback((event: any) => {
        const action = event.target.dataset.action;
        if (action) {
            setActivePage(action);
        }
    }, []);

    return <div className={'preferences'}>
        <div className={'preferences__content'}>
            <div className={'preferences__sidebar'}>
                {SIDEBAR_ACTIONS.map(action =>
                    <button key={'pref_' + action.action} data-action={action.action}
                            className={action.action === activePage ? 'selected' : ''}
                            onClick={handleSidebarAction}>{getIconByName(action.icon)}</button>)}
            </div>
            <div className={'preferences__panels'}>
                <Panel value={SidebarAction.Update} active={activePage}>
                    <div className={'card'}><TargetUpdateView config={config}></TargetUpdateView></div>
                </Panel>
                <Panel value={SidebarAction.User} active={activePage}>
                    <div className={'card'}><UserView config={config}></UserView></div>
                </Panel>
                <Panel value={SidebarAction.ApiServer} active={activePage}>
                    <div className={'card'}><ApiProxyInfoView config={config}></ApiProxyInfoView></div>
                </Panel>
                <Panel value={SidebarAction.MainConfig} active={activePage}>
                    <div className={'card'}><MainConfigView config={config}></MainConfigView></div>
                </Panel>
            </div>
        </div>
    </div>
}