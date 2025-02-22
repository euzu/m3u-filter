import {useTranslation} from "react-i18next";
import {useEffect, useMemo} from "react";
import {ReplaySubject, Subject} from "rxjs";

export type TranslateFunc = (key: string, variables?: Record<string, any>) => string;

export const LANGUAGE_LOCAL_STORAGE_KEY: string = "language"

export const LANGUAGE: Subject<string> = new ReplaySubject<string>(1);
export default function useTranslator(): (key: string, variables?: any) => string {
    const [t, i18n] = useTranslation('common');
    useEffect(() => {
        LANGUAGE.subscribe(lang => {
            localStorage.setItem(LANGUAGE_LOCAL_STORAGE_KEY, lang);
            i18n.changeLanguage(lang).then(() => {});
        });
    }, [i18n]);

    return useMemo((): TranslateFunc => t, [t]);
}
