import {Observable, of, from} from "rxjs";
function copyToClipboardOld(text: string): boolean {
    const el: HTMLElement = document.createElement('span');
    el.innerText = text;
    el.style.position = 'absolute';
    el.style.border = '0';
    el.style.padding = '0';
    el.style.margin = '0';
    // Move element out of screen horizontally
    el.style.position = 'absolute';
    const isRTL = document.documentElement.getAttribute('dir') === 'rtl';
    el.style[ isRTL ? 'right' : 'left' ] = '-9999px';
    // Move element to the same position vertically
    const yPosition = window.pageYOffset || document.documentElement.scrollTop;
    el.style.top = `${yPosition}px`;
    document.body.appendChild(el);

    const selection = window.getSelection();
    const range = document.createRange();
    range.selectNode(el);
    selection.removeAllRanges();
    selection.addRange(range);

    let result;
    try {
      result = document.execCommand('copy');
      selection.removeAllRanges();

    } catch(err) {
        result = false;
    }
    document.body.removeChild(el)
    return result;
}

export default function copyToClipboard(text: string): Observable<boolean> {
    let result: Observable<boolean>;
    if (copyToClipboardOld(text)) {
        result = of(true);
    } else {
        const promise = navigator.clipboard.writeText(text);
        result = new Observable<boolean>(observer => {
            from(promise).subscribe({
                next: () => observer.next(true),
                error: (err) => observer.next(false),
                complete: () => observer.complete()
            })
        });
    }
    return result;
}
