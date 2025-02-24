import React, {useCallback, useMemo, useState} from "react";
import './preferences.scss';
import ServerConfig from "../../model/server-config";
import UserView from "../user-view/user-view";
import TargetUpdateView from "../target-update-view/target-update-view";
import ApiProxyView from "../api-proxy-view/api-proxy-view";
import {getIconByName} from "../../icons/icons";
import Panel from "../panel/panel";
import MainConfigView from "../main-config-view/main-config-view";
import useTranslator from "../../hook/use-translator";

enum SidebarAction {
    Update = 'update',
    User = 'user',
    ApiServer = 'api_server',
    MainConfig = 'main_config'
}

const SIDEBAR_ACTIONS: { action: SidebarAction, icon: string, label: string }[] = [
    {action: SidebarAction.Update, icon: 'Refresh', label: 'LABEL.REFRESH'},
    {action: SidebarAction.User, icon: 'User', label: 'LABEL.USER'},
    {action: SidebarAction.ApiServer, icon: 'ApiServer', label: 'LABEL.PROXY'},
    {action: SidebarAction.MainConfig, icon: 'Config', label: 'LABEL.CONFIG'},
];

interface PreferencesProps {
    config: ServerConfig
}

export default function Preferences(props: PreferencesProps) {
    const {config} = props;
    const translate = useTranslator();
    const [activePage, setActivePage] = useState(SidebarAction.Update);
    const sidebar_actions = useMemo(() => SIDEBAR_ACTIONS.map(a => ({...a, label: translate(a.label)})), [translate])

    const handleSidebarAction = useCallback((event: any) => {
        const action = event.target.dataset.action;
        if (action) {
            setActivePage(action);
        }
    }, []);

    return <div className={'preferences'}>
        <div className={'preferences__content'}>
            <div className={'preferences__sidebar'}>
                {sidebar_actions.map(action =>
                    <div key={'pref_' + action.action} data-action={action.action}
                            className={'preferences__sidebar-menu-action' + (action.action === activePage ? ' selected' : '')}
                            onClick={handleSidebarAction}>{getIconByName(action.icon)}<label>{action.label}</label></div>)}
            </div>
            <div className={'preferences__panels'}>
                <Panel value={SidebarAction.Update} active={activePage}>
                    <div className={'card'}><TargetUpdateView config={config}></TargetUpdateView></div>
                </Panel>
                <Panel value={SidebarAction.User} active={activePage}>
                    <div className={'card'}><UserView config={config}></UserView></div>
                </Panel>
                <Panel value={SidebarAction.ApiServer} active={activePage}>
                    <div className={'card'}><ApiProxyView config={config}></ApiProxyView></div>
                </Panel>
                <Panel value={SidebarAction.MainConfig} active={activePage}>
                    <div className={'card'}><MainConfigView config={config}></MainConfigView></div>
                </Panel>
            </div>
            <div className={'preferences__sidebar'}></div>
        </div>
    </div>
}