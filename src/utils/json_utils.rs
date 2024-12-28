use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Error, Read, Write};
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{self, Deserializer, Value};

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
    next_obj.map_or_else(
        || Err(invalid_data("premature EOF")),
        |result| result.map_err(Into::into),
    )
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
pub fn json_iter_array<T: DeserializeOwned, R: Read>(
    mut reader: R,
) -> impl Iterator<Item = Result<T, io::Error>> {
    let mut at_start = false;
    std::iter::from_fn(move || yield_next_obj(&mut reader, &mut at_start).transpose())
}

pub fn json_filter_file(file_path: &Path, filter: &HashMap<&str, &str>) -> Vec<serde_json::Value> {
    let mut filtered: Vec<serde_json::Value> = Vec::new();
    if !file_path.exists() {
        return filtered; // Return early if the file does not exist
    }

    let Ok(file) = File::open(file_path) else {
        return filtered;
    };

    let reader = BufReader::new(file);
    for entry in json_iter_array::<serde_json::Value, BufReader<File>>(reader).flatten() {
        if let Some(item) = entry.as_object() {
            if filter.iter().all(|(&key, &value)| {
                item.get(key).is_some_and(|field_value| match field_value {
                    Value::String(s) => s == value,
                    Value::Number(n) => value.parse::<i64>().ok() == n.as_i64(),
                    _ => false,
                })
            }) {
                filtered.push(entry);
            }
        }
    }

    filtered
}

pub fn json_write_documents_to_file<T>(file: &Path, value: &T) -> Result<(), Error>
where
    T: ?Sized + Serialize,
{
    match File::create(file) {
        Ok(file) => {
            let mut writer = BufWriter::new(file);
            serde_json::to_writer(&mut writer, value)?;
            match writer.flush() {
                Ok(()) => Ok(()),
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

pub fn string_or_number_u32<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Value = serde::Deserialize::deserialize(deserializer)?;

    match value {
        Value::Number(num) => {
            if let Some(v) = num.as_u64() {
                u32::try_from(v)
                    .map_err(|_| serde::de::Error::custom("Number out of range for u32"))
            } else {
                Err(serde::de::Error::custom("Invalid number"))
            }
        }
        Value::String(s) => s
            .parse::<u32>()
            .map_err(|_| serde::de::Error::custom("Invalid string number")),
        _ => Err(serde::de::Error::custom("Expected number or string")),
    }
}

pub fn opt_string_or_number_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Value = serde::Deserialize::deserialize(deserializer)?;

    match value {
        Value::Null => Ok(None), // Handle null explicitly
        Value::Number(num) => {
            if let Some(v) = num.as_u64() {
                u32::try_from(v)
                    .map(Some)
                    .map_err(|_| serde::de::Error::custom("Number out of range for u32"))
            } else {
                Err(serde::de::Error::custom("Invalid number"))
            }
        }
        Value::String(s) => s
            .parse::<u32>()
            .map(Some)
            .map_err(|_| serde::de::Error::custom("Invalid string number")),
        _ => Err(serde::de::Error::custom("Expected number, string, or null")),
    }
}

pub fn string_or_number_f64<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Value = serde::Deserialize::deserialize(deserializer)?;

    match value {
        Value::Number(num) => num
            .as_f64()
            .ok_or_else(|| serde::de::Error::custom("Invalid number")),
        Value::String(s) => s
            .parse::<f64>()
            .map_err(|_| serde::de::Error::custom("Invalid string number")),
        _ => Err(serde::de::Error::custom("Expected number or string")),
    }
}

pub fn get_u64_from_serde_value(value: &Value) -> Option<u64> {
    match value {
        Value::Number(num_val) => num_val.as_u64(),
        Value::String(str_val) => match str_val.parse::<u64>() {
            Ok(val) => Some(val),
            Err(_) => None,
        },
        _ => None,
    }
}

pub fn get_u32_from_serde_value(value: &Value) -> Option<u32> {
    get_u64_from_serde_value(value).and_then(|val| u32::try_from(val).ok())
}

pub fn get_string_from_serde_value(value: &Value) -> Option<String> {
    match value {
        Value::Number(num_val) => num_val.as_i64().map(|num| num.to_string()),
        Value::String(str_val) => {
            if str_val.is_empty() {
                None
            } else {
                Some(str_val.clone())
            }
        }
        _ => None,
    }
}
