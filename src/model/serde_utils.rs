use serde::{Deserialize, Deserializer, Serializer};
use serde::de::DeserializeOwned;
use serde_json::Value;
use crate::model::config::ForceRedirect;

fn value_to_string_array(value: &[Value]) -> Vec<String> {
    value.iter().filter_map(value_to_string).collect()
}

fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) => Some(value.to_string()),
        _ => None,
    }
}

pub fn deserialize_as_option_rc_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;

    match &value {
        Value::String(s) => Ok(Some(s.to_owned())),
        Value::Number(s) => Ok(Some(s.to_string())),
        _ => Ok(None),
    }
}

pub fn deserialize_as_rc_string<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value: Value = Deserialize::deserialize(deserializer)?;

    match &value {
        Value::String(s) => Ok(s.to_string()),
        Value::Null => Ok(String::new()),
        _ => Ok(value.to_string()),
    }
}

pub fn deserialize_as_string_array<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Value::deserialize(deserializer).map(|v| match v {
        Value::String(value) => Some(vec![value]),
        Value::Array(value) => Some(value_to_string_array(&value)),
        _ => None,
    })
}


pub fn deserialize_number_from_string<'de, D, T: DeserializeOwned>(
    deserializer: D,
) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
{
    // we define a local enum type inside of the function
    // because it is untagged, serde will deserialize as the first variant
    // that it can
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum MaybeNumber<U> {
        // if it can be parsed as Option<T>, it will be
        Value(Option<U>),
        // otherwise try parsing as a string
        NumberString(String),
    }

    // deserialize into local enum
    let value: MaybeNumber<T> = Deserialize::deserialize(deserializer)?;
    match value {
        // if parsed as T or None, return that
        MaybeNumber::Value(value) => Ok(value),

        // (if it is any other string)
        MaybeNumber::NumberString(string) => {
            serde_json::from_str::<T>(string.as_str()).map_or_else(|_| Ok(None), |val| Ok(Some(val)))
        }
    }
}

pub fn deserialize_option_force_redirect<'de, D>(deserializer: D) -> Result<Option<ForceRedirect>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: Vec<String> = Vec::deserialize(deserializer)?;
    let mut flags = ForceRedirect::empty();

    for s in raw {
        match s.to_lowercase().as_str() {
            "live" => flags |= ForceRedirect::Live,
            "vod" => flags |= ForceRedirect::Vod,
            "series" => flags |= ForceRedirect::Series,
            unknown => return Err(serde::de::Error::custom(format!("Unknown ForceRedirect value: {unknown}. Valid values are 'live', 'vod', 'series'."))),
        }
    }

    Ok(Some(flags))
}

#[allow(clippy::ref_option)]
pub(crate) fn serialize_option_force_redirect<S>(force_redirect: &Option<ForceRedirect>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Some(flags) = force_redirect {
        let mut flag_strings = Vec::new();

        if flags.contains(ForceRedirect::Live) {
            flag_strings.push("live".to_string());
        }
        if flags.contains(ForceRedirect::Vod) {
            flag_strings.push("vod".to_string());
        }
        if flags.contains(ForceRedirect::Series) {
            flag_strings.push("series".to_string());
        }

        serializer.serialize_some(&flag_strings)
    } else {
        serializer.serialize_none()
    }
}

