import React, {useState, useRef, useEffect, JSX} from 'react';
import './split-panel.scss';

interface SplitPanelProps {
    top: JSX.Element;
    bottom: JSX.Element;
}

export const SplitPanel = (props: SplitPanelProps) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const [topHeight, setTopHeight] = useState(300);
    const [isDragging, setIsDragging] = useState(false);

    useEffect(() => {
        const handleMouseMove = (e: MouseEvent) => {
            if (!isDragging || !containerRef.current) return;
            const containerTop = containerRef.current.getBoundingClientRect().top;
            const newHeight = e.clientY - containerTop;
            setTopHeight(newHeight);
        };

        const stopDragging = () => setIsDragging(false);

        window.addEventListener('mousemove', handleMouseMove);
        window.addEventListener('mouseup', stopDragging);

        return () => {
            window.removeEventListener('mousemove', handleMouseMove);
            window.removeEventListener('mouseup', stopDragging);
        };
    }, [isDragging]);

    return (
        <div className="resizable-container" ref={containerRef}>
            <div className="top-pane" style={{ height: `${topHeight}px` }}>
                {props.top}
            </div>
            <div className="divider" onMouseDown={() => setIsDragging(true)}/>
            <div className="bottom-pane">
                {props.bottom}
            </div>
        </div>
    );
};
