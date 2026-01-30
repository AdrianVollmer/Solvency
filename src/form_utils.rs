/// Serde helpers for HTML form deserialization.
///
/// HTML `<select>` elements with an empty `<option value="">` send an empty
/// string for the field, which `serde_urlencoded` cannot parse as an integer.
/// These helpers treat empty strings as `None` for `Option<i64>` fields.
use serde::{Deserialize, Deserializer};

pub fn deserialize_optional_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s.as_deref() {
        None | Some("") => Ok(None),
        Some(v) => v.parse::<i64>().map(Some).map_err(serde::de::Error::custom),
    }
}
