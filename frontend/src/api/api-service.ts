import {Observable} from "rxjs";
import axios from "axios";
import config from "../config";
import ServiceContext from "../service/service-context";

const HEADER_CONTENT_TYPE = 'Content-Type';
const HEADER_LANGUAGE = 'X-Language';
const HEADER_ACCEPT = 'Accept';
const HEADER_AUTHORIZATION = 'Authorization';

export default interface ApiService {
    get<T>(query: string, url?: string): Observable<T>;

    post<T>(query: string, payload: any, url?: string): Observable<T>;

    put<T>(query: string, payload: any, url?: string): Observable<T>;

    delete<T>(query: string, url?: string): Observable<T>;

    postFile<T>(query: string, fileName: string, file: any, url?: string): Observable<T>;
    downloadFile<T>(query: string, payload: any, url?: string): Observable<T>;
}

export class DefaultApiService implements ApiService {

    private readonly DEFAULT_ERROR = {'origin': 'server', 'message': 'Server error'};

    private static getLanguage(): string {
        return "en";
    }

    protected getBaseUrl(): string {
        const apiBaseUrl = ServiceContext.config().getUiConfig().api.serverUrl;
        return apiBaseUrl.substring(0, apiBaseUrl.indexOf('/api/'));
    }

    private prepareError(err: any): any {
        return err || this.DEFAULT_ERROR;
    }

    private static getOption(options: any, key: any, defaultValue: any): string {
        if (options) {
            if (Object.prototype.hasOwnProperty.call(options, key)) {
                return options[key];
            }
        }
        return defaultValue;
    }

    protected getHeaders(options?: unknown): any {
        const headers: any = {};
        const language = DefaultApiService.getLanguage();
        if (language) {
            headers[HEADER_LANGUAGE] = language;
        }
        const value = DefaultApiService.getOption(options, HEADER_CONTENT_TYPE, 'application/json; charset=utf-8');
        if (value) {
            headers[HEADER_CONTENT_TYPE] = value;
        }
        headers[HEADER_ACCEPT] = 'application/json';
        const token = ServiceContext.auth().getToken();
        if (token) {
            headers[HEADER_AUTHORIZATION] = 'Bearer ' + token;
        }
        return headers;
    }

    protected getUrl(query: string, url?: string) {
        const apiBaseUrl = ServiceContext.config().getUiConfig().api.serverUrl;
        return (url ? url : apiBaseUrl) + query;
    }

    get<T>(query: string, url?: string): Observable<T> {
        return new Observable((observer) => {
            axios.get(this.getUrl(query, url), {headers: this.getHeaders()})
                .then((response) => {
                    observer.next(response.data);
                    observer.complete();
                })
                .catch((error) => observer.error(this.prepareError(error)));
        });
    }

    post<T>(query: string, payload: any, url?: string): Observable<T> {
        return new Observable((observer) => {
            axios.post<T>(this.getUrl(query, url), payload, {headers: this.getHeaders()})
                .then((response) => {
                    observer.next(response.data);
                    observer.complete();
                })
                .catch((error) => observer.error(this.prepareError(error)));
        });
    }

    put<T>(query: string, payload: any, url?: string): Observable<T> {
        return new Observable((observer) => {
            axios.put<T>(this.getUrl(query, url), payload, {headers: this.getHeaders()})
                .then((response) => {
                    observer.next(response.data);
                    observer.complete();
                })
                .catch((error) => observer.error(this.prepareError(error)));
        });
    }

    delete<T>(query: string, url?: string): Observable<T> {
        return new Observable((observer) => {
            axios.delete(this.getUrl(query, url), {headers: this.getHeaders()})
                .then((response) => {
                    observer.next(response.data);
                    observer.complete();
                })
                .catch((error) => observer.error(this.prepareError(error)));
        });
    }

    postFile<T>(query: string, fileName: string, file: any, url?: string): Observable<T> {
        const fd = new FormData();
        fd.append("fileName", fileName);
        fd.append("file", file, fileName);

        return new Observable((observer) => {
            axios.post<T>(this.getUrl(query, url), fd, {headers: this.getHeaders({[HEADER_CONTENT_TYPE]: undefined})})
                .then((response) => {
                    observer.next(response.data);
                    observer.complete();
                })
                .catch((error) => observer.error(this.prepareError(error)));
        });
    }

    downloadFile<T>(query: string, payload: any, url?: string): Observable<T> {
        return this.post(query, payload, url);
        // return new Observable((observer) => {
        //
        //     axios.post<T>(this.getUrl(query, url), payload, {responseType: 'blob', headers: this.getHeaders()})
        //         .then((response) => {
        //             const disposition = response.headers['content-disposition'];
        //             let filename = disposition.split(/;(.+)/)[1].split(/=(.+)/)[1];
        //             if (filename.toLowerCase().startsWith("utf-8''"))
        //                 filename = decodeURIComponent(filename.replace("utf-8''", ''));
        //             else
        //                 filename = filename.replace(/['"]/g, '');
        //             return {filename, data: response.data};
        //         }).then((params: { filename: string, data: any }) => {
        //         const url = window.URL.createObjectURL(params.data);
        //         const a = document.createElement('a');
        //         a.href = url;
        //         a.download = params.filename;
        //         document.body.appendChild(a); // append the element to the dom
        //         a.click();
        //         a.remove(); // afterwards, remove the element
        //         observer.next(true as any);
        //         observer.complete();
        //     }).catch((error) => observer.error(this.prepareError(error)));
        // });
    }

}
