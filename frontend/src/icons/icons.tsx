import createIcon from "../utils/icon-utils";
import * as React from "react";

const ICON: Record<string, React.JSX.Element> = {};

export const getIconByName = (name: string): any => {
    if (name) {
        // @ts-ignore
        return ICON[name];
    }
    return undefined;
}

const DEFAULT_ICONS = {
    Logo: 'M 6.808403,5 4,12.35462 11.145363,20 l 2.90033,-2.97444 c 0.51187,0.57826 0.85281,0.82576 2.51999,0.92977 1.09067,0.0677 1.93179,-0.0251 2.69123,-0.2964 0.50299,-0.17955 0.79531,-0.34443 1.29245,-0.726 0.54171,-0.41606 0.88103,-0.82067 1.22392,-1.45759 0.36995,-0.68744 0.51134,-1.13992 0.74086,-2.37439 0.23336,-1.25423 0.57747,-1.91554 1.24197,-2.38361 0.37439,-0.26379 0.34323,-0.31038 -0.27208,-0.40747 -1.02853,-0.162 -2.07477,0.11465 -3.06795,0.81307 -0.9144,0.64287 -1.39134,1.36224 -1.86026,2.80222 -0.55857,1.71139 -0.85428,2.24498 -1.53579,2.76886 -0.39088,0.30034 -0.5705,0.28994 -1.19691,0.20571 -0.41939,-0.0559 -0.79068,-0.31323 -1.09595,-0.57415 l 3.87192,-3.97086 -3.13648,-7.35462 -1.87287,3.09114 h -4.67945 z m 0.80935,7.00088 c 0.2965,-7.9e-4 0.6243,0.0613 0.98239,0.21288 2.04007,0.86382 1.82962,5.78592 1.82962,5.78592 l -4.65784,-4.80064 c -0.11895,-0.0251 0.5618,-1.1929 1.84583,-1.19644 z m 7.17781,0 c 1.28403,0.003 1.96671,1.1735 1.84763,1.19827 l -4.65963,4.80063 c 0,0 -0.21053,-4.9221 1.8296,-5.78591 0.35811,-0.15157 0.68608,-0.21361 0.9824,-0.21288 z',
    ContentCopy: 'M16 1H4c-1.1 0-2 .9-2 2v14h2V3h12V1zm3 4H8c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h11c1.1 0 2-.9 2-2V7c0-1.1-.9-2-2-2zm0 16H8V7h11v14z',
    DeleteSweep: 'M15 16h4v2h-4zm0-8h7v2h-7zm0 4h6v2h-6zM3 18c0 1.1.9 2 2 2h6c1.1 0 2-.9 2-2V8H3v10zM14 5h-3l-1-1H6L5 5H2v2h12z',
    Delete: 'M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z',
    Search: 'M15.5 14h-.79l-.28-.27C15.41 12.59 16 11.11 16 9.5 16 5.91 13.09 3 9.5 3S3 5.91 3 9.5 5.91 16 9.5 16c1.61 0 3.09-.59 4.23-1.57l.27.28v.79l5 4.99L20.49 19l-4.99-5zm-6 0C7.01 14 5 11.99 5 9.5S7.01 5 9.5 5 14 7.01 14 9.5 11.99 14 9.5 14z',
    ClearSearch: 'M 10.82534,3 C 7.3479487,3 4.5022347,5.716944 4.2480387,9.1748729 H 6.3014596 C 6.5556707,6.859298 8.4664087,5.0582909 10.82534,5.0582909 c 2.531786,0 4.575514,2.068585 4.575514,4.6311538 0,2.5625733 -2.043728,4.6311543 -4.575514,4.6311543 -0.172856,0 -0.335527,-0.03153 -0.508391,-0.05226 v 2.080402 c 0.172857,0.02062 0.335527,0.03015 0.508391,0.03015 1.637017,0 3.14233,-0.60752 4.30146,-1.616081 l 0.274054,0.289447 v 0.812061 L 20.484758,21 22,19.466331 16.926025,14.320604 h -0.802303 l -0.28597,-0.277387 c 0.996446,-1.173226 1.596663,-2.696845 1.596663,-4.3537672 0,-3.6946315 -2.958832,-6.6894447 -6.609075,-6.6894447 z M 2.7228671,11.048238 2,11.777886 4.5121637,14.320599 2,16.863312 l 0.7228671,0.729648 2.5101775,-2.540703 2.5121641,2.540703 0.720882,-0.729648 -2.510178,-2.542713 2.510178,-2.542713 -0.720882,-0.729648 -2.5121641,2.542713 z',
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
    Download: 'M5 20h14v-2H5v2zM19 9h-4V3H9v6H5l7 7 7-7z',
    Config: 'M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58c.18-.14.23-.41.12-.61l-1.92-3.32c-.12-.22-.37-.29-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54c-.04-.24-.24-.41-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58c-.18.14-.23.41-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z',
    PersonAdd: 'M15 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm-9-2V7H4v3H1v2h3v3h2v-3h3v-2H6zm9 4c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z',
    PersonRemove: 'M14 8c0-2.21-1.79-4-4-4S6 5.79 6 8s1.79 4 4 4 4-1.79 4-4zm3 2v2h6v-2h-6zM2 18v2h16v-2c0-2.66-5.33-4-8-4s-8 1.34-8 4z',
    WebSearch: 'M19.3 16.9c.4-.7.7-1.5.7-2.4 0-2.5-2-4.5-4.5-4.5S11 12 11 14.5s2 4.5 4.5 4.5c.9 0 1.7-.3 2.4-.7l3.2 3.2 1.4-1.4-3.2-3.2zm-3.8.1c-1.4 0-2.5-1.1-2.5-2.5s1.1-2.5 2.5-2.5 2.5 1.1 2.5 2.5-1.1 2.5-2.5 2.5zM12 20v2C6.48 22 2 17.52 2 12S6.48 2 12 2c4.84 0 8.87 3.44 9.8 8h-2.07c-.64-2.46-2.4-4.47-4.73-5.41V5c0 1.1-.9 2-2 2h-2v2c0 .55-.45 1-1 1H8v2h2v3H9l-4.79-4.79C4.08 10.79 4 11.38 4 12c0 4.41 3.59 8 8 8z',
    CheckMark: 'M9 16.17 4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z',
    Checked: 'M 4.2222224,2 C 3,2 2,3.000021 2,4.2222362 V 19.777764 C 2,20.999979 3,22 4.2222224,22 H 19.777778 C 20.999989,22 22,20.999979 22,19.777764 V 4.2222362 C 22,3.000021 20.999989,2 19.777778,2 Z m 0,2.2222362 H 19.777778 V 19.777764 H 4.2222224 Z M 17.288622,7.1063517 10.441833,13.953134 6.7113668,10.233533 5.2465223,11.698352 10.441833,16.89369 18.753467,8.5820472 Z',
    Hourglass: 'M6 2v6h.01L6 8.01 10 12l-4 4 .01.01H6V22h12v-5.99h-.01L18 16l-4-4 4-3.99-.01-.01H18V2H6zm10 14.5V20H8v-3.5l4-4 4 4zm-4-5-4-4V4h8v3.5l-4 4z',
    Error: 'M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z',
    Regexp: 'M 14.219989,2 V 6.6779027 L 10.218308,4.3991767 8.9526346,6.5626756 13.00229,8.8690562 8.8806751,11.216304 l 1.2656739,2.16227 4.07364,-2.3199 v 4.62413 h 2.532593 v -4.677903 l 3.98081,2.26705 L 22,11.108144 18.066228,8.8681345 21.985982,6.6358063 20.720308,4.4735364 16.752582,6.7332118 V 2 Z M 4.8198059,16.438384 C 3.2775726,16.438384 1.9999995,17.698564 2,19.219808 1.9999995,20.741043 3.2775726,22 4.8198059,22 c 1.542234,0 2.8185603,-1.258957 2.8185597,-2.780192 6e-7,-1.521244 -1.2763257,-2.781424 -2.8185597,-2.781424 z',
    User: 'M12 12c2.21 0 4-1.79 4-4s-1.79-4-4-4-4 1.79-4 4 1.79 4 4 4zm0 2c-2.67 0-8 1.34-8 4v2h16v-2c0-2.66-5.33-4-8-4z',
    Refresh: 'M17.65 6.35C16.2 4.9 14.21 4 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08c-.82 2.33-3.04 4-5.65 4-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z',
    ApiServer: 'm20.2 5.9.8-.8C19.6 3.7 17.8 3 16 3s-3.6.7-5 2.1l.8.8C13 4.8 14.5 4.2 16 4.2s3 .6 4.2 1.7zm-.9.8c-.9-.9-2.1-1.4-3.3-1.4s-2.4.5-3.3 1.4l.8.8c.7-.7 1.6-1 2.5-1 .9 0 1.8.3 2.5 1l.8-.8zM19 13h-2V9h-2v4H5c-1.1 0-2 .9-2 2v4c0 1.1.9 2 2 2h14c1.1 0 2-.9 2-2v-4c0-1.1-.9-2-2-2zM8 18H6v-2h2v2zm3.5 0h-2v-2h2v2zm3.5 0h-2v-2h2v2z',
    Warn: 'M 11.999765,2 2,22 h 20 z m 0,4.1999619 6.845654,13.6948891 H 5.1545815 Z m -0.909027,4.2211761 v 5.263416 h 1.818524 v -5.263416 z m 0,6.31599 v 2.105149 h 1.818524 v -2.105149 z',
    Gallery: 'M 11.999742,2 C 6.5002947,2 1.9999999,6.5002944 2,11.999742 2,17.499188 6.5002947,22 11.999742,22 17.499187,22 22,17.499188 22,11.999742 22,6.5002944 17.499187,1.9999999 11.999742,2 Z M 7.4796784,5.5010206 H 9.2940081 C 10.454244,5.5013381 11.3944,6.4424302 11.393587,7.6026664 v 1.8143297 c -3.18e-4,1.1594289 -0.94015,2.0992499 -2.0995789,2.0995789 H 7.4796784 C 6.3194422,11.517395 5.3783618,10.577232 5.3780327,9.4169961 V 7.6026664 C 5.3772125,6.4416227 6.3186347,5.5002073 7.4796784,5.5010206 Z m 7.2263136,0 h 1.816396 c 1.160237,3.175e-4 2.100392,0.9414098 2.099579,2.1016458 v 1.8143297 c -3.17e-4,1.1594289 -0.94015,2.0992499 -2.099579,2.0995789 h -1.816396 c -1.159429,-3.18e-4 -2.09925,-0.94015 -2.099579,-2.0995789 V 7.6026664 c -8.2e-4,-1.1602362 0.939342,-2.1013166 2.099579,-2.1016458 z M 7.4796784,12.483425 h 1.8143297 c 1.1594289,3.18e-4 2.0992499,0.94015 2.0995789,2.099579 v 1.816397 c -3.18e-4,1.159428 -0.94015,2.099249 -2.0995789,2.099578 H 7.4796784 C 6.3194422,18.4998 5.3783618,17.559637 5.3780327,16.399401 v -1.816397 c 3.175e-4,-1.160236 0.9414095,-2.100391 2.1016457,-2.099579 z m 7.2263136,0 h 1.816396 c 1.159429,3.18e-4 2.09925,0.94015 2.099579,2.099579 v 1.816397 c -3.17e-4,1.159428 -0.94015,2.099249 -2.099579,2.099578 h -1.816396 c -1.159429,-3.17e-4 -2.09925,-0.94015 -2.099579,-2.099578 v -1.816397 c 3.17e-4,-1.159429 0.94015,-2.09925 2.099579,-2.099579 z',
    Editor: 'M8 5v14l11-7z',
    ScheduleAdd: 'm 15.427906,2 0.0026,4.3103469 -4.402514,-0.00424 0.0016,2.4026963 4.402514,0.00475 0.0026,4.2565168 2.402815,0.0026 -0.0021,-4.2565172 4.164457,0.00517 -0.0016,-2.403213 -4.164974,-0.00465 -0.0026,-4.3103469 z M 9.2342365,7.5143019 c -3.9841557,5e-7 -7.2343587,3.2535111 -7.2343586,7.2428491 0,3.989335 3.2502029,7.242849 7.2343586,7.242849 3.9841545,0 7.2322915,-3.253514 7.2322915,-7.242849 0,-0.06325 -0.0014,-0.126255 -0.0031,-0.18912 H 14.68171 c 0.0022,0.06278 0.0041,0.125793 0.0041,0.18912 0,3.025716 -2.429816,5.460169 -5.4516058,5.460169 -3.0217902,0 -5.4531561,-2.434453 -5.4531561,-5.460169 0,-3.025718 2.4313659,-5.4586186 5.4531561,-5.4586186 0.1480969,0 0.2937091,0.00764 0.4387121,0.019119 C 9.6657863,8.738416 9.6404593,8.1019918 9.6781163,7.5292876 9.531058,7.5202214 9.3835267,7.5143032 9.2342349,7.5143032 Z M 8.078806,10.651303 v 5.534575 h 5.462975 V 14.71943 H 9.5432468 v -4.068127 z',
    ScheduleRemove: 'm 14.051424,1.9998779 -1.742213,1.697054 3.122965,3.0458089 -3.191235,3.1104043 1.741155,1.6975709 3.191765,-3.1098876 3.084332,3.0080846 L 21.999878,9.7518594 18.916075,6.7432575 21.935312,3.8023519 20.194157,2.1042643 17.17439,5.0462036 Z M 9.4085202,7.51427 c -4.0802372,6e-7 -7.4086424,3.253631 -7.4086423,7.242969 0,3.989334 3.3284051,7.242968 7.4086423,7.242968 4.0802358,0 7.4070548,-3.253634 7.4070548,-7.242968 0,-0.06325 -0.0014,-0.126271 -0.0032,-0.189136 h -1.824773 c 0.0023,0.06278 0.0042,0.125808 0.0042,0.189136 0,3.025716 -2.488677,5.460131 -5.5833398,5.460131 -3.0946634,0 -5.5843983,-2.434415 -5.5843983,-5.460131 0,-3.025718 2.4897349,-5.458582 5.5843983,-5.458582 0.1516683,0 0.3008134,0.00764 0.4493133,0.01912 C 9.8504755,8.7385417 9.8245025,8.10196 9.8630655,7.5292556 9.712521,7.5201898 9.5614122,7.51427 9.4085202,7.51427 Z m -1.1833506,3.137276 v 5.534546 H 13.820153 V 14.719515 H 9.7249975 v -4.067969 z',
    Logout: 'm17 7-1.41 1.41L18.17 11H8v2h10.17l-2.58 2.58L17 17l5-5zM4 5h8V3H4c-1.1 0-2 .9-2 2v14c0 1.1.9 2 2 2h8v-2H4z',
    SelectAll: 'M 2,9.2105263 H 13.578947 V 11.315789 H 2 Z M 2,5 H 13.578947 V 7.105263 H 2 Z m 0,8.421054 h 7.368421 v 2.105263 H 2 Z m 18.515789,-2.178949 -4.473684,4.46316 -2.231579,-2.23158 -1.48421,1.484211 3.715789,3.726316 L 22,12.726317 Z',
    DeselectAll: 'M 2,5 V 7.1538458 H 13.612371 V 5 Z M 2,9.3076922 V 11.461539 H 13.612371 V 9.3076922 Z m 14.082473,1.7899618 -1.265978,1.392431 2.327832,2.559791 -2.327832,2.557693 L 16.082473,19 18.408247,16.442307 20.734022,19 22,17.607569 19.674225,15.049876 22,12.490085 20.734022,11.097654 18.408247,13.655347 Z M 2,13.615385 v 2.153847 h 7.3896896 v -2.153847 z',
    Visibility: 'M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5M12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5m0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3',
    Edit: 'M3 17.25V21h3.75L17.81 9.94l-3.75-3.75zM20.71 7.04c.39-.39.39-1.02 0-1.41l-2.34-2.34a.996.996 0 0 0-1.41 0l-1.83 1.83 3.75 3.75z',
    Calendar: 'M19 4h-1V2h-2v2H8V2H6v2H5c-1.11 0-1.99.9-1.99 2L3 20c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V6c0-1.1-.9-2-2-2m0 16H5V10h14zM9 14H7v-2h2zm4 0h-2v-2h2zm4 0h-2v-2h2zm-8 4H7v-2h2zm4 0h-2v-2h2zm4 0h-2v-2h2z',
    Unlimited: 'M18.6 6.62c-1.44 0-2.8.56-3.77 1.53L12 10.66 10.48 12h.01L7.8 14.39c-.64.64-1.49.99-2.4.99-1.87 0-3.39-1.51-3.39-3.38S3.53 8.62 5.4 8.62c.91 0 1.76.35 2.44 1.03l1.13 1 1.51-1.34L9.22 8.2C8.2 7.18 6.84 6.62 5.4 6.62 2.42 6.62 0 9.04 0 12s2.42 5.38 5.4 5.38c1.44 0 2.8-.56 3.77-1.53l2.83-2.5.01.01L13.52 12h-.01l2.69-2.39c.64-.64 1.49-.99 2.4-.99 1.87 0 3.39 1.51 3.39 3.38s-1.52 3.38-3.39 3.38c-.9 0-1.76-.35-2.44-1.03l-1.14-1.01-1.51 1.34 1.27 1.12c1.02 1.01 2.37 1.57 3.82 1.57 2.98 0 5.4-2.41 5.4-5.38s-2.42-5.37-5.4-5.37',
    Today: 'M16.53 11.06 15.47 10l-4.88 4.88-2.12-2.12-1.06 1.06L10.59 17zM19 3h-1V1h-2v2H8V1H6v2H5c-1.11 0-1.99.9-1.99 2L3 19c0 1.1.89 2 2 2h14c1.1 0 2-.9 2-2V5c0-1.1-.9-2-2-2m0 16H5V8h14z',
    Help: 'M11 18h2v-2h-2zm1-16C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2m0 18c-4.41 0-8-3.59-8-8s3.59-8 8-8 8 3.59 8 8-3.59 8-8 8m0-14c-2.21 0-4 1.79-4 4h2c0-1.1.9-2 2-2s2 .9 2 2c0 2-3 1.75-3 5h2c0-2.25 3-2.5 3-5 0-2.21-1.79-4-4-4'
}


Object.entries(DEFAULT_ICONS).forEach(icon_def => {
    ICON[icon_def[0]] = createIcon(icon_def[0], icon_def[1]);
});


export default ICON;
