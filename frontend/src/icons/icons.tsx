import createIcon from "../utils/icon-utils";
import * as React from "react";

const ICON: Record<string, React.JSX.Element> = {};

export const getIconByName = (name: string): any => {
    if (name) {
        // @ts-ignore
        return ICON[name];
    }
    return null;
}

const DEFAULT_ICONS = {
    ContentCopy: 'M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z',
    DeleteSweep: 'M15 16h4v2h-4zm0-8h7v2h-7zm0 4h6v2h-6zM3 18c0 1.1.9 2 2 2h6c1.1 0 2-.9 2-2V8H3v10zM14 5h-3l-1-1H6L5 5H2v2h12z',
    Search: 'M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z',
    ExpandMore: 'M16.59 8.59 12 13.17 7.41 8.59 6 10l6 6 6-6z',
    ExpandLess: 'm12 8-6 6 1.41 1.41L12 10.83l4.59 4.58L18 14z',
    ChevronRight: 'M10 6 8.59 7.41 13.17 12l-4.58 4.59L10 18l6-6z',
    LinkRounded: 'M17 7h-3c-.55 0-1 .45-1 1s.45 1 1 1h3c1.65 0 3 1.35 3 3s-1.35 3-3 3h-3c-.55 0-1 .45-1 1s.45 1 1 1h3c2.76 0 5-2.24 5-5s-2.24-5-5-5zm-9 5c0 .55.45 1 1 1h6c.55 0 1-.45 1-1s-.45-1-1-1H9c-.55 0-1 .45-1 1zm2 3H7c-1.65 0-3-1.35-3-3s1.35-3 3-3h3c.55 0 1-.45 1-1s-.45-1-1-1H7c-2.76 0-5 2.24-5 5s2.24 5 5 5h3c.55 0 1-.45 1-1s-.45-1-1-1z',
    PlayArrow: 'M8 5v14l11-7z',
    ArrowLeft: 'm14 7-5 5 5 5V7z',
    ArrowRight: 'm10 17 5-5-5-5v10z',
    ArrowDown: 'm7 10 5 5 5-5z',
    ArrowUp: 'm7 14 5-5 5 5z',
    CloudDownload: 'M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96zM17 13l-5 5-5-5h3V9h4v4h3z',
    'Download': 'M5 20h14v-2H5v2zM19 9h-4V3H9v6H5l7 7 7-7z'


}


Object.entries(DEFAULT_ICONS).forEach(icon_def => {
    ICON[icon_def[0]] = createIcon(icon_def[0], icon_def[1]);
});


export default ICON;
