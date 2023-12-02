import React, {useCallback, useEffect, useState} from "react";
import './user-view.scss';
import ServerConfig, {TargetUser} from "../../model/server-config";
import {getIconByName} from "../../icons/icons";
import TextGenerator from "../../utils/text-generator";
import {useSnackbar} from "notistack";
import {useServices} from "../../provider/service-provider";
import ConfigUtils from "../../utils/config-utils";
import TabSet, {TabSetTab} from "../tab-set/tab-set";
import TagSelect from "../tag-select/tags-select";

const PROXY_OPTIONS = [
    { value: 'reverse', label: 'Reverse' },
    { value: 'redirect', label: 'Redirect' }
];

interface UserViewProps {
    config: ServerConfig;
}

export default function UserView(props: UserViewProps) {
    const {config} = props;
    const services = useServices();
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const [targets, setTargets] = useState<TargetUser[]>([]);
    const [activeTarget, setActiveTarget] = useState<string>(undefined);
    const [tabs, setTabs] = useState<TabSetTab[]>([]);

    useEffect(() => {
        if (config) {
            const target_names = ConfigUtils.getTargetNames(config);
            const missing = config?.api_proxy?.user.filter(target => !target_names.includes(target.target));
            const result: TargetUser[] = target_names?.map(name => ({
                src: true,
                target: name,
                credentials: config.api_proxy.user.find(t => t.target === name)?.credentials || []
            } as any));
            missing?.forEach(target => {
                result.push({src: false, target: target.target, credentials: target.credentials} as any);
            });
            const targets = result || [];
            setTargets(targets);
            if (targets?.length) {
                setActiveTarget(targets[0].target);
            }
            setTabs(targets.map(target => ({key: target.target, label: target.target})));
        }
    }, [config])

    const handleUserAdd = useCallback((evt: any) => {
        const target_name = evt.target.dataset.target;
        const target = targets.find(target => target.target === target_name);
        if (target) {
            const usernameExists = (uname: string): boolean => {
                for (const target of targets) {
                    if (target.credentials.find(c => c.username === uname)) {
                        return true;
                    }
                }
                return false;
            };
            let cnt = 0;
            let username = TextGenerator.generateUsername().toLowerCase();
            while (usernameExists(username)) {
                username = TextGenerator.generateUsername().toLowerCase();
                cnt++;
                if (cnt > 1000) {
                    username = "";
                    break;
                }
            }
            target.credentials.push({
                username,
                password: TextGenerator.generatePassword(),
                token: TextGenerator.generatePassword(),
                proxy: 'reverse'
            });
            setTargets([...targets]);
        }
    }, [targets]);

    const handleUserRemove = useCallback((evt: any) => {
        const idx = evt.target.dataset.idx;
        const target_name = evt.target.dataset.target;
        const target = targets.find(target => target.target === target_name);
        if (target) {
            target.credentials.splice(idx, 1);
            setTargets([...targets]);
        }
    }, [targets]);

    const handleValueChange = useCallback((evt: any) => {
        const target_name = evt.target.dataset.target;
        const target = targets.find(target => target.target === target_name);
        if (target) {
            const idx = evt.target.dataset.idx;
            const field: any = evt.target.dataset.field;
            if (field === 'username') {
                target.credentials[idx].username = evt.target.value;
            } else if (field === 'password') {
                target.credentials[idx].password = evt.target.value;
            } else if (field === 'token') {
                target.credentials[idx].token = evt.target.value;
            }
        }
    }, [targets]);

    const handleChange = useCallback((fieldWithTargetAndIndex: string, value: any) => {
        const sep_idx = fieldWithTargetAndIndex.lastIndexOf('-');
        const target_name = fieldWithTargetAndIndex.substring(0, sep_idx);
        const target = targets.find(target => target.target === target_name);
        if (target) {
            const idx = parseInt(fieldWithTargetAndIndex.substring(sep_idx+1));
            target.credentials[idx].proxy = value;
        }
    }, [targets]);

    const handleSave = useCallback(() => {
        const usernames: any = {};
        for (const target of targets) {
            for (const user of target.credentials) {
                if (!user.username?.trim().length) {
                    enqueueSnackbar("Username empty!", {variant: 'error'});
                    return;
                }
                if (usernames[user.username]) {
                    enqueueSnackbar("Duplicate Username! " + user.username, {variant: 'error'});
                    return;
                }
                usernames[user.username] = true;
            }
        }
        const targetUser = targets.map(t => {
            t.credentials.forEach(c => {
                c.username = c.username.trim();
                c.password = c.password.trim();
                c.token = c.token?.trim();
            })
            return {target: t.target, credentials: t.credentials}
        });
        services.config().saveTargetUser(targetUser).subscribe({
            next: () => enqueueSnackbar("User saved!", {variant: 'success'}),
            error: (err) => enqueueSnackbar("Failed to save user!", {variant: 'error'})
        });
    }, [targets, services, enqueueSnackbar]);

    const handleTabChange = useCallback((target: string) => {
        setActiveTarget(target);
    }, []);

    return <div className={'user'}>

        <div className={'user__toolbar'}><label>User</label>
            <button title={'Save'} onClick={handleSave}>Save</button>
        </div>
        <TabSet tabs={tabs} active={activeTarget} onTabChange={handleTabChange}></TabSet>
        <div className={'user__content'}>
            <div className={'user__content-targets'}>
                {
                    targets?.map(target => <div key={target.target}
                                                className={'user__target' + (activeTarget !== target.target ? ' hidden' : '')}>
                        <div className={'user__target-target'}>
                            <label className={(target as any).src ? '' : 'target-not-exists'}>{target.target}</label>
                            <div className={'user__target-target-toolbar'}>
                                <button title={'New User'} data-target={target.target}
                                        onClick={handleUserAdd}>{getIconByName('PersonAdd')}</button>
                            </div>
                        </div>

                        <div className={'user__target-user-table-container'}>
                            <div className={'user__target-user-table'}>
                                <div className={'user__target-user-row user__target-user-table-header'}>
                                    <div className={'user__target-user-col user__target-user-col-header'}>
                                        <label>Username</label></div>
                                    <div className={'user__target-user-col user__target-user-col-header'}>
                                        <label>Password</label></div>
                                    <div className={'user__target-user-col user__target-user-col-header'}>
                                        <label>Token</label></div>
                                    <div className={'user__target-user-col user__target-user-col-header'}></div>
                                </div>

                                {target.credentials.map((usr, idx) =>
                                    <div key={'credential' + idx} className={'user__target-user-row'}>
                                        {['username', 'password', 'token'].map((field) =>
                                            <div key={'target_' + target.target + '_' + field + '_' + usr.username}
                                                 className={'user__target-user-col'}>
                                                <div className={'user__target-user-col-label'}><label>{field.charAt(0).toUpperCase() + field.slice(1)}</label>
                                                </div>
                                                <input data-target={target.target} data-idx={idx}
                                                       defaultValue={(usr as any)[field]}
                                                       data-field={field} onChange={handleValueChange}></input>
                                            </div>
                                        )}
                                        <div key={'target_' + target.target + '_proxy_' + usr.username}
                                             className={'user__target-user-col'}>
                                            <div className={'user__target-user-col-label'}><label>Proxy</label></div>

                                            <TagSelect options={PROXY_OPTIONS} name={target.target + '-' + idx}
                                                       defaultValues={(usr as any)?.['proxy']} multi={false} onSelect={handleChange}></TagSelect>
                                        </div>

                                        <div className={'user__target-user-col user__target-user-col-toolbar'}>
                                            <span data-target={target.target} data-idx={idx} onClick={handleUserRemove}>
                                                {getIconByName('PersonRemove')}
                                            </span>
                                        </div>
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>)}
            </div>
        </div>
    </div>
}