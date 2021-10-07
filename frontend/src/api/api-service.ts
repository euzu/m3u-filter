import {Observable} from "rxjs";
import axios from "axios";
import config from "../config";

const HEADER_CONTENT_TYPE = 'Content-Type';
const HEADER_LANGUAGE = 'X-Language';
const HEADER_ACCEPT = 'Accept';

export default interface ApiService {
    get<T>(query: string, url?: string): Observable<T>;
    post<T>(query: string, payload: any, url?: string): Observable<T>;
    put<T>(query: string, payload: any, url?: string): Observable<T>;
    delete<T>(query: string, url?: string) : Observable<T>;
    postFile<T>(query: string, fileName: string, file: any, url?: string) : Observable<T>;
}

export class DefaultApiService implements ApiService {

    private baseUrl: string = config.api.serverUrl;

    private readonly DEFAULT_ERROR = {'origin': 'server', 'message': 'Server error'};

    private static getLanguage(): string {
        return "de_DE";
    }

    private prepareError(err: any): any {
        return err || this.DEFAULT_ERROR;
    }

    private static getOption(options: any, key: any, defaultValue: any): string {
        if (options) {
            if (options.hasOwnProperty(key)) {
                return options[key];
            }
        }
        return defaultValue;
    }

    protected getHeaders(options?: {}): any {
        let headers: any = {};
        let language = DefaultApiService.getLanguage();
        if (language) {
            headers[HEADER_LANGUAGE] = language;
        }
        let value = DefaultApiService.getOption(options, HEADER_CONTENT_TYPE, 'application/json; charset=utf-8');
        if (value) {
            headers[HEADER_CONTENT_TYPE] = value;
        }
        headers[HEADER_ACCEPT] = 'application/json';
        return headers;
    }

    protected getUrl(query: string, url?: string) {
        return (url ? url : this.baseUrl) + query;
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
        let fd = new FormData();
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

}
