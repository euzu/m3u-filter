import {Observable, zip} from "rxjs";
import i18next from "i18next";
import {initReactI18next} from "react-i18next";
import Fetcher from "./fetcher";
import {LANGUAGE, LANGUAGE_LOCAL_STORAGE_KEY} from "../hook/use-translator";

export default function i18n_init(languages: string[]): Observable<boolean> {
    return new Observable<boolean>(observer => {
        const failed = (err: any) => {
            console.error('failed to load i18n');
            observer.error(err);
        }

        i18next.use(initReactI18next)
        zip(languages.map(lang => Fetcher.fetchJson('/i18n/' + lang + '_common.json')))
            .subscribe({
                next: (responses) => {
                    let savedLanguage = localStorage.getItem(LANGUAGE_LOCAL_STORAGE_KEY);
                    if (savedLanguage?.length ) {
                        LANGUAGE.next(savedLanguage);
                    } else {
                        LANGUAGE.next(languages[0]);
                    }
                    // 'common' is our custom namespace
                    const resources = languages.reduce((acc: any, lang, idx) => {
                        acc[lang] = {common: responses[idx]};
                        return acc;
                    }, {});
                    i18next.init({
                        interpolation: {escapeValue: false},  // React already does escaping
                        lng: 'en',                            // language to use
                        resources,
                    }).then(() => observer.next(true))
                        .catch(failed);
                },
                error: failed
            });
    });
}