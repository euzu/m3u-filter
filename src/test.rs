#[cfg(test)]
mod tests {
    use crate::filter::get_filter;

    #[test]
    fn test_filter() {
        let flt1 = r#"(Group ~ "A" OR Group ~ "B") AND (Name ~ "C" OR Name ~ "D" OR Name ~ "E") OR (NOT (Title ~ "F") AND NOT Title ~ "K")"#;
        match get_filter(flt1, None) {
            Ok(filter) => {
                assert_eq!(format!("{filter}"), flt1);
            },
            Err(e) => {
                panic!("{}", e)
            }
        }

        let flt2 = r#"Group ~ "d" AND ((Name ~ "e" AND NOT ((Name ~ "c" OR Name ~ "f"))) OR (Name ~ "a" OR Name ~ "b"))"#;
        match get_filter(flt2, None) {
            Ok(filter) => {
                assert_eq!(format!("{filter}"), flt2);
            },
            Err(e) => {
                panic!("{}", e)
            }
        }
    }

}