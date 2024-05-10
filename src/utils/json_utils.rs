use std::collections::HashMap;
use std::fs::File;
use serde::de::DeserializeOwned;
use serde_json::{self, Deserializer};
use std::io::{self, BufReader, BufWriter, Error, Read, Write};
use std::path::Path;
use serde::Serialize;

fn read_skipping_ws(mut reader: impl Read) -> io::Result<u8> {
    loop {
        let mut byte = 0u8;
        reader.read_exact(std::slice::from_mut(&mut byte))?;
        if !byte.is_ascii_whitespace() {
            return Ok(byte);
        }
    }
}

fn invalid_data(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

fn deserialize_single<T: DeserializeOwned, R: Read>(reader: R) -> io::Result<T> {
    let next_obj = Deserializer::from_reader(reader).into_iter::<T>().next();
    match next_obj {
        Some(result) => result.map_err(Into::into),
        None => Err(invalid_data("premature EOF")),
    }
}

fn yield_next_obj<T: DeserializeOwned, R: Read>(
    mut reader: R,
    at_start: &mut bool,
) -> io::Result<Option<T>> {
    if *at_start {
        match read_skipping_ws(&mut reader)? {
            b',' => deserialize_single(reader).map(Some),
            b']' => Ok(None),
            _ => Err(invalid_data("`,` or `]` not found")),
        }
    } else {
        *at_start = true;
        if read_skipping_ws(&mut reader)? == b'[' {
            // read the next char to see if the array is empty
            let peek = read_skipping_ws(&mut reader)?;
            if peek == b']' {
                Ok(None)
            } else {
                deserialize_single(io::Cursor::new([peek]).chain(reader)).map(Some)
            }
        } else {
            Err(invalid_data("`[` not found"))
        }
    }
}

// https://stackoverflow.com/questions/68641157/how-can-i-stream-elements-from-inside-a-json-array-using-serde-json
pub(crate) fn json_iter_array<T: DeserializeOwned, R: Read>(
    mut reader: R,
) -> impl Iterator<Item=Result<T, io::Error>> {
    let mut at_start = false;
    std::iter::from_fn(move || yield_next_obj(&mut reader, &mut at_start).transpose())
}

pub(crate) fn json_filter_file(file_path: &Path, filter: &HashMap<&str, &str>) -> Vec<serde_json::Value> {
    let mut filtered: Vec<serde_json::Value> = Vec::new();
    if file_path.exists() {
        if let Ok(file) = File::open(file_path) {
            let reader = BufReader::new(file);
            for entry in json_iter_array::<serde_json::Value, BufReader<File>>(reader).flatten() {
                if let Some(item) = entry.as_object() {
                    for (&key, &value) in filter {
                        if let Some(field_value) = item.get(key) {
                            if field_value.is_string() && field_value.eq(value) {
                                filtered.push(entry.clone());
                            } else if let Some(num_val) = field_value.as_i64() {
                                if let Ok(filter_num_val) = value.parse::<i64>() {
                                    if num_val == filter_num_val {
                                        filtered.push(entry.clone());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    filtered
}

pub(crate) fn json_write_documents_to_file<T>(file: &Path, value: &T) -> Result<(), Error>
    where
        T: ?Sized + Serialize {
    match File::create(file) {
        Ok(file) => {
            let mut writer = BufWriter::new(file);
            serde_json::to_writer(&mut writer, value)?;
            match writer.flush() {
                Ok(()) => Ok(()),
                Err(e) => Err(e)
            }
        }
        Err(e) => Err(e)
    }
}