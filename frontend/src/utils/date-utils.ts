function formatDate(date: any): string {
    if (typeof date === 'string') {
        return date;
    }
    const day = String(date.getDate()).padStart(2, '0');
    const month = String(date.getMonth() + 1).padStart(2, '0');
    const year = date.getFullYear();

    return `${year}-${month}-${day}`;
}

const DateUtils = {
    formatDate: (date: any) => {
        if (date) {
            console.log(date);
            return formatDate(date);
        }
        return '';
    }
}

export default DateUtils;