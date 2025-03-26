import {first, Observable} from "rxjs";

const fetchJson = (fileName: string): Observable<any> => {
    return new Observable((observer) => {
        fetch(process.env.PUBLIC_URL + fileName, {method: 'GET'})
            .then(res => res.json())
            .then(data => {
                observer.next(data)
                observer.complete();
            })
            .catch((e) => observer.error(e));
    }).pipe(first());
}

const Fetcher = {
    fetchJson,
}

export default Fetcher;
