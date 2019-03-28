use std::time::Duration;

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

use crate::Result;

pub fn read_line(line: Vec<u8>) -> Result<()> {
    println!("{}", String::from_utf8_lossy(&line));
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct Line {
    #[serde(serialize_with = "as_base64", deserialize_with = "from_base64")]
    data: Vec<u8>,
    timestamp: Duration,
}

fn as_base64<T, S>(key: &T, serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    T: AsRef<[u8]>,
    S: Serializer,
{
    serializer.serialize_str(&base64::encode(key.as_ref()))
}

fn from_base64<'de, D>(deserializer: D) -> std::result::Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    let bytes = base64::decode(&string).map_err(de::Error::custom)?;
    Ok(bytes)
}
