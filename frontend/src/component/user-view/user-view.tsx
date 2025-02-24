import React, {useCallback, useEffect, useRef, useState} from "react";
import './user-view.scss';
import ServerConfig, {Credentials, TargetUser} from "../../model/server-config";
import {getIconByName} from "../../icons/icons";
import {useSnackbar} from "notistack";
import {useServices} from "../../provider/service-provider";
import ConfigUtils from "../../utils/config-utils";
import TabSet, {TabSetTab} from "../tab-set/tab-set";
import useTranslator from "../../hook/use-translator";
import DateUtils from "../../utils/date-utils";
import PlaylistFilter from "../playlist-filter/playlist-filter";
import UserEditor from "../user-editor/user-editor";
import TextGenerator from "../../utils/text-generator";

const COLUMNS = [
    {field: 'username', label: 'LABEL.USERNAME'},
    {field: 'password', label: 'LABEL.PASSWORD', hidden: true},
    {field: 'token', label: 'LABEL.TOKEN', hidden: true},
    {field: 'server', label: 'LABEL.SERVER'},
    {field: 'proxy', label: 'LABEL.PROXY'},
    {field: 'max_connections', label: 'LABEL.MAX_CON'},
    {field: 'status', label: 'LABEL.STATUS'},
    {field: 'exp_date', label: 'LABEL.EXP_DATE', render: (value: any, hidden?: boolean) => DateUtils.formatDate(value)},
]

COLUMNS.forEach(col => {
    if (!col.render) {
        col.render = (value: any, hidden?: boolean) => value ? (hidden ? '***' : '' + value) : '';
    }
})

const prepareCredentials = (targetUser: TargetUser[]) => {
    targetUser.forEach((user) => {
        user.credentials.forEach((credential) => {
            if (credential.exp_date) {
                credential.exp_date = new Date(credential.exp_date * 1000) as any;
            }
        })
    });
}

const prepareTargetUserForSave = (targetUser: TargetUser[]): TargetUser[] => {
    return targetUser.map((user) => {
        let newUser = {...user, credentials: user.credentials.map(c => ({...c}))};
        newUser.credentials.forEach((credential) => {
            if (credential.exp_date) {
                credential.exp_date = Math.floor((credential.exp_date as any).getTime() / 1000);
            }
        });
        return newUser;
    });
}

const usernameExists = (uname: string, targets: TargetUser[]): boolean => {
    for (const target of targets) {
        if (target.credentials.find(c => c.username === uname)) {
            return true;
        }
    }
    return false;
};

const checkuser = (user: Credentials): string | undefined => {
    if (!user.username?.trim().length) {
        return "MESSAGES.USER.USERNAME_REQUIRED";
    }
    // eslint-disable-next-line eqeqeq
    if (user.max_connections != undefined) {
        const max_con = parseInt(user.max_connections as any);
        // eslint-disable-next-line eqeqeq
        if (isNaN(max_con) || max_con < 0 || (('' + max_con) != user.max_connections as any)) {
            return 'MESSAGES.USER.MAX_CONNECTIONS_INVALID';
        } else {
            if (max_con < 1) {
                user.max_connections = undefined;
            } else {
                user.max_connections = max_con;
            }
        }
    }
    return undefined;
}


const createNewUser = (targets: TargetUser[]): Credentials => {
    let cnt = 0;
    let username = TextGenerator.generateUsername().toLowerCase();
    while (usernameExists(username, targets)) {
        username = TextGenerator.generateUsername().toLowerCase();
        cnt++;
        if (cnt > 1000) {
            username = "";
            break;
        }
    }
    const created_at = Math.floor(Date.now() / 1000);
    return {
            username,
            password: TextGenerator.generatePassword(),
            token: TextGenerator.generatePassword(),
            proxy: 'reverse',
            created_at,
            exp_date: undefined,
            max_connections: undefined,
            status: "Active",
            // @ts-ignore
            _ref: undefined, // an indicator for new user
    };
}

interface UserViewProps {
    config: ServerConfig;
}

export default function UserView(props: UserViewProps) {
    const {config} = props;
    const services = useServices();
    const translate = useTranslator();
    const userEditorRef = useRef(null);
    const {enqueueSnackbar/*, closeSnackbar*/} = useSnackbar();
    const [serverOptions, setServerOptions] = useState<{ value: string, label: string }[]>([]);
    const [targets, setTargets] = useState<TargetUser[]>([]);
    const [activeTarget, setActiveTarget] = useState<string>(undefined);
    const [tabs, setTabs] = useState<TabSetTab[]>([]);
    const [showHiddenFields, setShowHiddenFields] = useState<Record<string, boolean>>({});
    const [filteredUser, setFilteredUser] = useState<Record<string, {
        filter: string,
        regexp: boolean,
        user: Credentials[]
    }>>({});

    useEffect(() => {
        if (config) {
            const serverOptions = config?.api_proxy?.server?.map(serverInfo => ({
                value: serverInfo.name,
                label: serverInfo.name
            }));
            setServerOptions(serverOptions || []);
            const target_names = ConfigUtils.getTargetNames(config);
            const missing = config?.api_proxy?.user.filter(target => !target_names.includes(target.target));
            const result: TargetUser[] = target_names?.map(name => ({
                src: true,
                target: name,
                credentials: config.api_proxy.user.find(t => t.target === name)?.credentials || []
            } as any));
            prepareCredentials(result);
            missing?.forEach(target => {
                result.push({src: false, target: target.target, credentials: target.credentials} as any);
            });
            const targets: TargetUser [] = result || [];
            setTargets(targets);
            if (targets?.length) {
                setActiveTarget(targets[0].target);
                const hiddenFields = COLUMNS.filter(c => c.hidden);
                const hidden: any = {};
                targets.forEach(target => target.credentials.forEach(c => {
                    if (!c.server) {
                        c.server = serverOptions?.[0]?.value;
                    }
                    hiddenFields.forEach(h => {
                        hidden[c.username + h.field] = true
                    });
                }));
                setShowHiddenFields(hidden);
            }
            setTabs(targets.map(target => ({key: target.target, label: target.target})));
        }
    }, [config])

    const handleUserAdd = useCallback((evt: any) => {
        const target_name = evt.target.dataset.target;
        const target = targets.find(target => target.target === target_name);
        if (target) {
            const user = createNewUser(targets);
            userEditorRef.current.edit(user, target_name);
        }
    }, [targets]);

    const handleUserRemove = useCallback((evt: any) => {
        const username = evt.target.dataset.user;
        const target_name = evt.target.dataset.target;
        const target = targets.find(target => target.target === target_name);
        if (target) {
            let idx = target.credentials.findIndex(c => c.username === username)
            if (idx >= 0) {
                target.credentials.splice(idx, 1);
                setTargets([...targets]);
            }
        }
    }, [targets]);

    const handleUserEdit = useCallback((evt: any) => {
        const username = evt.target.dataset.user;
        const target_name = evt.target.dataset.target;
        const target = targets.find(target => target.target === target_name);
        if (target) {
            const user = target.credentials.find(c => c.username === username);
            if (user) {
                userEditorRef.current.edit({...user, _ref: user.username}, target_name);
            }
        }
    }, [targets]);

    const handleSave = useCallback(() => {
        const usernames: any = {};
        for (const target of targets) {
            for (const user of target.credentials) {
                const err = checkuser(user);
                if (err) {
                    enqueueSnackbar(translate(err), {variant: 'error'});
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
        const toSaveTargetUser = prepareTargetUserForSave(targetUser);
        services.config().saveTargetUser(toSaveTargetUser).subscribe({
            next: () => enqueueSnackbar(translate('MESSGES.SAVE.USER.SUCCESS'), {variant: 'success'}),
            error: (err) => enqueueSnackbar(translate('MESSGES.SAVE.USER.FAIL'), {variant: 'error'})
        });
    }, [targets, services, enqueueSnackbar, translate]);

    const handleVisibility = useCallback((evt: any) => {
        let key = evt.target.dataset.hiddenkey;
        setShowHiddenFields((hiddenFields: any) => ({...hiddenFields, [key]: !hiddenFields[key]}));
    }, []);

    const handleFilter = useCallback((filter: string, regexp: boolean): void => {
        let filter_value = regexp ? filter : filter.toLowerCase();
        const target = targets.find(t => t.target === activeTarget);

        if (target) {
            if (filter_value?.length) {
                const filtered = target.credentials.filter((credential: Credentials) => {
                    if (regexp) {
                        // eslint-disable-next-line eqeqeq
                        return credential.username.trim().match(filter_value) != undefined;
                    } else {
                        return (credential.username.trim().toLowerCase().indexOf(filter_value) > -1);
                    }
                }) ?? [];
                setFilteredUser((filteredUser: any) => ({
                    ...filteredUser,
                    [activeTarget]: {filter, regexp, user: filtered}
                }));
            } else {
                setFilteredUser((filteredUser: any) => ({
                    ...filteredUser, [activeTarget]: undefined
                }));
            }

        }
    }, [activeTarget, targets]);

    const handleUserEditorSubmit = useCallback((user: Credentials, target_name: string): boolean => {
        const userRef = (user as any)._ref;
        const err = checkuser(user);
        if (err) {
            enqueueSnackbar(translate(err), {variant: 'error'});
            return false;
        }
        if ((userRef && user.username === userRef) || !usernameExists(user.username, targets)) {
            const target = targets.find(target => target.target === target_name);
            if (target) {
                delete (user as any)._ref;
                if (userRef) {
                    const index = target.credentials.findIndex(c => c.username === userRef);
                    target.credentials.splice(index, 1, user);
                } else {
                    target.credentials.push(user);
                }
                setTargets(targets.slice());
                return true;
            } else {
                enqueueSnackbar(translate("MESSAGES.USER.TARGET_NOT_FOUND") + target_name, {variant: 'error'});
            }
        } else {
            enqueueSnackbar(translate("MESSAGES.USER.DUPLICATE_USERNAME") + user.username, {variant: 'error'});
        }
        return false;
    }, [targets, translate, enqueueSnackbar]);

    return <div className={'user'}>

        <div className={'user__toolbar'}><label>{translate('LABEL.USER')}</label>
            <PlaylistFilter onFilter={handleFilter} options={filteredUser[activeTarget]}></PlaylistFilter>
            <button title={translate('LABEL.SAVE')} onClick={handleSave}>{translate('LABEL.SAVE')}</button>
        </div>
        <TabSet tabs={tabs} active={activeTarget} onTabChange={setActiveTarget}></TabSet>
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
                                    {COLUMNS.map(col => <div key={col.field}
                                                             className={'user__target-user-col user__target-user-col-header'}>
                                        <label>{translate(col.label)}</label></div>)}
                                    <div className={'user__target-user-col user__target-user-col-header'}></div>
                                </div>

                                {(filteredUser[target.target]?.user ?? target.credentials).map((usr, idx) =>
                                    <div key={'credential' + idx} className={'user__target-user-row'}>
                                        {COLUMNS.map(c => <div
                                            key={'target_' + target.target + '_' + c.field + '_' + usr.username}
                                            className={'user__target-user-col'}>
                                            <div className={'user__target-user-col-label'}>
                                                <label>{translate(c.label)}</label>
                                            </div>
                                            <div className={'user__target-user-col-value'}>{c.hidden &&
                                                <span className={'visibility'} data-hiddenkey={usr.username + c.field}
                                                      onClick={handleVisibility}>
                                                {getIconByName('Visibility')}</span>}
                                                {c.render((usr as any)[c.field], (showHiddenFields[usr.username + c.field] ?? c.hidden))}
                                            </div>
                                        </div>)
                                        }
                                        <div className={'user__target-user-col-toolbar'}>
                                            <span data-target={target.target} data-user={usr.username}
                                                  onClick={handleUserRemove}>
                                                {getIconByName('PersonRemove')}
                                            </span>
                                            <span data-target={target.target} data-user={usr.username}
                                                  onClick={handleUserEdit}>
                                                {getIconByName('Edit')}
                                            </span>
                                        </div>
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>)}
            </div>
        </div>
        <UserEditor onSubmit={handleUserEditorSubmit} ref={userEditorRef} serverOptions={serverOptions}></UserEditor>
    </div>
}