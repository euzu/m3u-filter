import React, {useCallback, useState, useRef} from 'react';
import './tree.scss';
import {PlaylistGroup, PlaylistItem} from "../../model/playlist";
import {ExpandMore, ChevronRight} from "@material-ui/icons";

export type TreeState = {[key: number] : boolean};

interface TreeProps {
    data: PlaylistGroup[];
    state: TreeState;
}

export default function Tree(props: TreeProps) {

    const [_, setForceUpdate] = useState(null);
    const {state, data} = props;
    const expanded = useRef<TreeState>({});

    const handleChange = useCallback((event: any) => {
        const key = event.target.dataset.group;
        state[key] = !state[key];
        setForceUpdate({});
    }, [state]);

    const handleExpand = useCallback((event: any) => {
        const key = event.target.dataset.group;
        expanded.current[key] = !expanded.current[key];
        setForceUpdate({});
    }, []);

    const renderEntry = useCallback((entry: PlaylistItem, index: number): React.ReactNode => {
        return <div key={entry.id} className={'tree-channel'}><div className={'tree-channel-nr'}>{index+1}</div>{entry.header.name}</div>
        //<TreeItem key={entry.id} nodeId={entry.id} label={entry.header.name}/>
    }, []);

    const renderGroup = useCallback((group: PlaylistGroup): React.ReactNode => {
        return <div className={'tree-group'} key={group.id}>
            <div className={'tree-group-header'}>
                <div className={'tree-expander'} data-group={group.id} onClick={handleExpand}>{expanded.current[group.id] ? <ExpandMore/> : <ChevronRight/>}</div>
                <input type={"checkbox"} onChange={handleChange} data-group={group.id}/>
                {group.title} <div className={'tree-group-count'}>({group.channels.length})</div>
            </div>
            { expanded.current[group.id] && (
            <div className={'tree-group-childs'}>
                {group.channels.map(renderEntry)}
            </div>)}
        </div>;
    }, [handleChange, handleExpand, renderEntry]);

    const renderPlaylist = useCallback((): React.ReactNode => {
        if (!data) {
            return null;
        }
        return <React.Fragment>
            {data.map(renderGroup)}
        </React.Fragment>;
    }, [data, renderGroup]);

    return <div className={'tree'}>{renderPlaylist()}</div>;
} 