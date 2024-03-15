#[cfg(test)]
mod tests {
    use crate::filter::get_filter;
    use crate::model::model_xtream::MultiXtreamMapping;
    use crate::repository::xtream_repository::{read_xtream_mapping, write_xtream_mapping};

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


    // #[test]
    // fn test_xtream_id_mapping() {
    //     let mappings = vec![
    //         MultiXtreamMapping { stream_id: 2, input_id: 3 },
    //         MultiXtreamMapping { stream_id: 4, input_id: 5 },
    //         MultiXtreamMapping { stream_id: 8, input_id: 6 },
    //     ];
    //
    //     write_xtream_mapping(&mappings).unwrap();
    //     for i in 1..=mappings.len() {
    //         let mapping = read_xtream_mapping(i as u32).unwrap().unwrap();
    //         let test_mapping = mappings.get(i-1).unwrap();
    //         assert_eq!(mapping.stream_id, test_mapping.stream_id);
    //         assert_eq!(mapping.input_id, test_mapping.input_id);
    //     }
    // }
}