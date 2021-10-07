import React from 'react';
import './progress.scss';

interface ProgressProps {
    visible: boolean;
}

export default function Progress(props: ProgressProps) {
    
    return <div className={'progress' + (props.visible ? ' progress-visible' : '')}>
        <div className="spinner">
            <div></div>
            <div></div>
            <div></div>
        </div>
    </div>;
}