import React, {ReactNode} from "react";
import './loading-indicator.scss';

interface LoadingIndicatorProps {
    loading: boolean;
}

export default function LoadingIndicator(props: LoadingIndicatorProps): ReactNode {
    if (!props.loading) {
        return <div className="loading-bar-placeholder"></div>;
    }
    return <div className="loading-bar-container">
        <div className="loading-bar"></div>
    </div>;
}