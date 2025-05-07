import React, {JSX, useCallback, useEffect, useRef, useState} from "react";
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
import UserEditor, {STATUS_OPTIONS} from "../user-editor/user-editor";
import TextGenerator from "../../utils/text-generator";

const renderExpDate = (value: any, hidden?: boolean) => {
    if (!value) {
        return getIconByName('Unlimited');
    }
    return DateUtils.formatDate(value)
};

const renderBool = (value: any, hidden?: boolean) => {
    return <span
        className={'checkbox-' + (value ? 'checked' : 'unchecked')}>{getIconByName(value ? 'CheckMark' : 'Clear')}</span>;
};

const renderComment = (value: any, hidden?: boolean) => {
    if (value?.length) {
        return <span className={'user__target-user-col-comment'}>{value}</span>;
    }
    return undefined;
};

const renderMaxCon = (value: any, hidden?: boolean) => {
    if (!value) {
        return getIconByName('Unlimited');
    }
    return value;
};

const renderStatus = (value: any, hidden?: boolean) => {
    if (value) {
        return <span className={'status-' + value.toLowerCase()}>{value}</span>
    }
    return value;
};

const renderProxyType = (value: any, hidden?: boolean) => {
    if (value) {
        return <span className={'proxy-type-' + value.toLowerCase()}>{value}</span>
    }
    return value;
};

const COLUMNS = [
    {field: 'username', label: 'LABEL.USERNAME'},
    {field: 'password', label: 'LABEL.PASSWORD', hidden: true},
    {field: 'token', label: 'LABEL.TOKEN', hidden: true},
    {field: 'server', label: 'LABEL.SERVER'},
    {field: 'proxy', label: 'LABEL.PROXY', render: renderProxyType},
    {field: 'max_connections', label: 'LABEL.MAX_CON', render: renderMaxCon},
    {field: 'status', label: 'LABEL.STATUS', render: renderStatus},
    {field: 'exp_date', label: 'LABEL.EXP_DATE', render: renderExpDate},
    {field: 'ui_enabled', label: 'LABEL.UI_ENABLED', render: renderBool},
    {field: 'comment', label: 'LABEL.NOTES', render: renderComment, action: true},
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
                credential.exp_date = DateUtils.unixSecondsToDate(credential.exp_date);
            }
            if (credential.created_at) {
                credential.created_at = DateUtils.unixSecondsToDate(credential.created_at);
            }
        })
    });
}

const prepareTargetUserForSave = (targetUser: TargetUser[]): TargetUser[] => {
    return targetUser.map((user) => {
        let storeUser = {...user, credentials: user.credentials.map(c => ({...c}))};
        storeUser.credentials.forEach((credential) => {
            credential.exp_date = DateUtils.toUnixSeconds(credential.exp_date);
            credential.created_at = DateUtils.toUnixSeconds(credential.created_at);
        });
        return storeUser;
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
        return "MESSAGES.SAVE.USER.USERNAME_REQUIRED";
    }
    // eslint-disable-next-line eqeqeq
    if (user.max_connections != undefined) {
        const max_con = parseInt(user.max_connections as any);
        // eslint-disable-next-line eqeqeq
        if (isNaN(max_con) || max_con < 0 || (('' + max_con) != user.max_connections as any)) {
            return 'MESSAGES.SAVE.USER.MAX_CONNECTIONS_INVALID';
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
    const created_at = Date.now();
    const exp_date = new Date();
    exp_date.setFullYear(exp_date.getFullYear() + 1);
    return {
        username,
        password: TextGenerator.generatePassword(),
        token: TextGenerator.generatePassword(),
        proxy: 'reverse',
        created_at,
        exp_date: exp_date.getTime(),
        max_connections: 1,
        status: "Active",
        ui_enabled: true,
        comment: undefined,
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
    const [targetOptions, setTargetOptions] = useState<Record<string, any>>({});
    const [activeTarget, setActiveTarget] = useState<string>(undefined);
    const [tabs, setTabs] = useState<TabSetTab[]>([]);
    const [showHiddenFields, setShowHiddenFields] = useState<Record<string, boolean>>({});
    const [filteredUser, setFilteredUser] = useState<Record<string, {
        filter: string,
        regexp: boolean,
        status: undefined,
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
            const targetOptions = config.sources
                ?.flatMap(s => s.targets || [])
                ?.reduce((acc: any, t: any) => {
                    acc[t.name] = t.options || {};
                    return acc;
                }, {});
            setTargetOptions(targetOptions);
            const result: TargetUser[] = target_names?.map(name => ({
                src: true,
                target: name,
                credentials: config.api_proxy?.user.find(t => t.target === name)?.credentials || []
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
        const tokens: any = {};
        for (const target of targets) {
            for (const user of target.credentials) {
                const err = checkuser(user);
                if (err) {
                    enqueueSnackbar(translate(err), {variant: 'error'});
                    return;
                }
                if (usernames[user.username]) {
                    enqueueSnackbar(translate("MESSAGES.USER.DUPLICATE_USERNAME") + user.username, {variant: 'error'});
                    return;
                }
                usernames[user.username] = true;
                if (user.token) {
                    if (tokens[user.token]) {
                        enqueueSnackbar(translate("MESSAGES.USER.DUPLICATE_TOKEN") + user.token, {variant: 'error'});
                        return;
                    }
                    tokens[user.token] = true;
                }
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
            next: () => enqueueSnackbar(translate('MESSAGES.SAVE.USER.SUCCESS'), {variant: 'success'}),
            error: (err) => enqueueSnackbar(translate('MESSAGES.SAVE.USER.FAIL'), {variant: 'error'})
        });
    }, [targets, services, enqueueSnackbar, translate]);

    const handleVisibility = useCallback((evt: any) => {
        let key = evt.target.dataset.hiddenkey;
        setShowHiddenFields((hiddenFields: any) => ({...hiddenFields, [key]: !hiddenFields[key]}));
    }, []);

    const filterTarget = useCallback((target: TargetUser, filter: string, regexp: boolean, filterForStatus: string): void => {
        if (target) {
            if (filter?.length) {
                let filtered = target.credentials.filter((credential: Credentials) => {
                    if (regexp) {
                        // eslint-disable-next-line eqeqeq
                        return credential.username.trim().match(filter) != undefined;
                    } else {
                        return (credential.username.trim().toLowerCase().indexOf(filter) > -1);
                    }
                }) ?? [];
                if (filterForStatus) {
                    filtered = filtered.filter((c: Credentials) => c.status === filterForStatus);
                }
                setFilteredUser((filteredUser: any) => ({
                    ...filteredUser,
                    [target.target]: {filter, regexp, status: filterForStatus, user: filtered}
                }));
            } else {
                let filtered = undefined;
                if (filterForStatus) {
                    filtered = {
                        filter: undefined,
                        regexp: false,
                        status: filterForStatus,
                        user: target.credentials.filter((c: Credentials) => c.status === filterForStatus)
                    };
                }
                setFilteredUser((filteredUser: any) => ({
                    ...filteredUser, [target.target]: filtered
                }));
            }
        }
    }, []);

    const handleFilter = useCallback((filter: string, regexp: boolean): void => {
        let filter_value = regexp ? filter : filter?.toLowerCase();
        const target = targets.find(t => t.target === activeTarget);
        if (target) {
            let currentFilter = filteredUser[target.target];
            filterTarget(target, filter_value, regexp, currentFilter?.status);
        }
    }, [activeTarget, targets, filterTarget, filteredUser]);

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

    const renderTargetOptions = useCallback((target: string): JSX.Element => {
        let options = targetOptions?.[target];
        if (options?.force_redirect?.length > 2) {
            return <div
                className={'user__target-target-options'}>{translate('HINT.CONFIG.USER.TARGET_FORCE_REDIRECT')} {options.force_redirect}</div>;
        }
        return <></>;
    }, [targetOptions, translate]);

    const handleStatusFilter = useCallback((evt: any) => {
        const target_name = evt.target.dataset.target;
        let target = targets.find((t) => t.target === target_name);
        if (target) {
            let value = evt.target.value;
            value = value?.length ? value : undefined;
            let currentFilter = filteredUser[target_name];
            filterTarget(target, currentFilter?.filter, !!currentFilter?.regexp, value)
        }
    }, [filteredUser, filterTarget, targets]);

    const handleColumnAction = useCallback((evt: any) => {
        const field = evt.target.dataset.field;
        if (field === 'comment') {
            const target_name = evt.target.dataset.target;
            let target = targets.find((t) => t.target === target_name);
            if (target) {
                const username = evt.target.dataset.username;
                const user = target.credentials.find(usr => usr.username === username);
                if (user) {
                    const dialog: any = document.getElementById("comment-dialog");
                    const comment = document.getElementById("comment-text");
                    comment.textContent = user.comment;
                    dialog.showModal();
                }
            }
        }
    }, [targets]);

    return <div className={'user'}>
        <div className={'user__toolbar'}><label>{translate('LABEL.TARGETS')} / {translate('LABEL.USERS')}</label>
            <PlaylistFilter onFilter={handleFilter} options={filteredUser[activeTarget]}></PlaylistFilter>
            <button data-tooltip='LABEL.SAVE' onClick={handleSave}>{translate('LABEL.SAVE')}</button>
        </div>
        <TabSet tabs={tabs} active={activeTarget} onTabChange={setActiveTarget}></TabSet>
        <div className={'user__content'}>
            <div className={'user__content-targets'}>
                {
                    targets?.map(target => <div key={target.target}
                                                className={'user__target' + (activeTarget !== target.target ? ' hidden' : '')}>
                        <div className={'user__target-target'}>
                            <label>
                                {!(target as any).src &&
                                    <span
                                        className={'target-not-exists'}>{translate('MESSAGES.TARGET_NOT_EXISTS')}</span>}
                                {renderTargetOptions(target.target)}
                            </label>

                            <div className={'label'}>{translate("LABEL.STATUS")}</div>
                            <select data-target={target.target} onChange={handleStatusFilter} defaultValue={undefined}>
                                <option value={undefined}></option>
                                {STATUS_OPTIONS.map(l => <option key={target.target + l.value}
                                                                 value={l.value}>{l.label}</option>)}
                            </select>

                            <div className={'user__target-target-toolbar'}>
                                <button data-tooltip={'New User'} data-target={target.target}
                                        onClick={handleUserAdd}>{getIconByName('PersonAdd')}</button>
                            </div>
                        </div>

                        <div className={'user__target-user-table-container'}>
                            <div className={'user__target-user-table'}>
                                <div className={'user__target-user-row user__target-user-table-header'}>
                                    <div
                                        className={'user__target-user-col user__target-user-col-header user__target-user-col-header-tools'}></div>
                                    {COLUMNS.map(col => <div key={col.field}
                                                             className={'user__target-user-col user__target-user-col-header'}>
                                        <label>{translate(col.label)}</label></div>)}
                                </div>

                                {(filteredUser[target.target]?.user ?? target.credentials).map((usr, idx) =>
                                    <div key={'credential' + idx} className={'user__target-user-row'}>
                                        <div className={'user__target-user-col'}>
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
                                        {COLUMNS.map(c => <div
                                            key={'target_' + target.target + '_' + c.field + '_' + usr.username}
                                            className={'user__target-user-col'}>
                                            <div className={'user__target-user-col-label'}>
                                                <label>{translate(c.label)}</label>
                                            </div>
                                            <div
                                                className={'user__target-user-col-value' + (c.action ? ' user__target-user-col-action' : '')}
                                                data-username={usr.username}
                                                data-field={c.field}
                                                data-target={activeTarget}
                                                onClick={c.action ? handleColumnAction : undefined}>{c.hidden &&
                                                <span className={'visibility'} data-hiddenkey={usr.username + c.field}
                                                      onClick={handleVisibility}>
                                                {getIconByName('Visibility')}</span>}
                                                {c.render((usr as any)[c.field], (showHiddenFields[usr.username + c.field] ?? c.hidden))}
                                            </div>
                                        </div>)
                                        }
                                    </div>
                                )}
                            </div>
                        </div>
                    </div>)}
            </div>
        </div>
        <UserEditor onSubmit={handleUserEditorSubmit} ref={userEditorRef} serverOptions={serverOptions}></UserEditor>
        <dialog id="comment-dialog" className={'comment-dialog'}>
            <p id="comment-text"></p>
            <button className={'button'}
                    onClick={() => (document.getElementById("comment-dialog") as any).close()}>{translate('LABEL.CLOSE')}</button>
        </dialog>
    </div>
}