#[cfg(test)]
mod tests {
    use crate::filter::get_filter;

    #[test]
    fn test_filter() {
        let flt1 = "(Group ~ \"A\" OR Group ~ \"B\") AND (Name ~ \"C\" OR Name ~ \"D\" OR Name ~ \"E\") OR (NOT (Title ~ \"F\") AND NOT Title ~ \"K\")";
        match get_filter(flt1, None) {
            Ok(filter) => {
                assert_eq!(format!("{}", filter), flt1);
            },
            Err(_e) => {}
        }
    }

}