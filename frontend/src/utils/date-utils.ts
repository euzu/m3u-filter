function formatDate(date: any): string {
    if (typeof date === 'string') {
        return date;
    }
    if (typeof date === 'number') {
        date = new Date(date);

    }
    if (typeof date?.getTime === 'function') {
        const day = String(date.getDate()).padStart(2, '0');
        const month = String(date.getMonth() + 1).padStart(2, '0');
        const year = date.getFullYear();

        return `${year}-${month}-${day}`;
    }
    return '';
}

const DateUtils = {
    formatDate: (date: any) => {
        if (date) {
            return formatDate(date);
        }
        return '';
    },
    toUnixSeconds: (date: any) => {
        if (date && typeof date.getTime == 'function') {
            if (date.getTime() > 0) {
                return Math.floor(date.getTime() / 1000);
            }
            return 0;
        }
        if (date && typeof date == 'number') {
            if ( date > 0) {
                return Math.floor(date / 1000);
            }
            return 0;
        }
        return date;
    },
    unixSecondsToDate: (date: any) => {
        if (date && typeof date == 'number') {
            return new Date(date * 1000);
        }
        if (date && typeof date.getTime == 'function') {
            return date;
        }
        return date;
    }
}

export default DateUtils;