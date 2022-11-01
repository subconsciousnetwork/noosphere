use anyhow::{anyhow, Error};
use cid::Cid;
use serde::{
    de::{self, Deserializer, MapAccess, SeqAccess, Visitor},
    ser::{SerializeStruct, Serializer},
    Deserialize, Serialize,
};

use std::{
    fmt,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// An [NSRecord] is a struct representing a record stored in the
/// Noosphere Name System's DHT containing a [Cid] of the
/// result, as well as a TTL expiration as seconds from Unix epoch.
///
/// # Serialization
///
/// When transmitting records across the network, they're encoded
/// as JSON-formatted UTF-8 strings. The [NSRecord::to_bytes]
/// and [NSRecord::from_bytes] methods respectively handle the
/// serialization and deserialization in this format.
///
/// Fields are mapped to the corresponding properties and JSON types:
///
/// * `cid` => `"cid" as String`
/// * `expires` => `"exp" as Number`
///
/// An example of the serialized payload structure and
/// conversion looks like:
///  
/// ```
/// use noosphere_ns::{Cid, NSRecord};
/// use std::str::FromStr;
///
/// let cid_str = "bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy";
/// let expires = 1667262626;
///
/// let record = NSRecord::new(Cid::from_str(cid_str).unwrap(), expires);
/// assert_eq!(record.cid.to_string(), cid_str);
/// assert_eq!(record.expires, expires);
///
/// let bytes = record.to_bytes().unwrap();
/// let record_str = "{\"cid\":\"bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy\",\"exp\":1667262626}";
/// assert_eq!(&String::from_utf8(bytes.clone()).unwrap(), record_str);
/// assert_eq!(NSRecord::from_bytes(bytes).unwrap(), record);
/// ```
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct NSRecord {
    /// The link to the resolved sphere's content.
    pub cid: Cid,
    /// TTL expiration time as seconds from Unix epoch.
    pub expires: u64,
}

impl NSRecord {
    /// Creates a new [NSRecord].
    pub fn new(cid: Cid, expires: u64) -> Self {
        Self { cid, expires }
    }

    /// Creates a new [NSRecord] with an expiration `ttl` seconds from now.
    pub fn new_from_ttl(cid: Cid, ttl: u64) -> Result<Self, Error> {
        let expires: u64 = SystemTime::now()
            .checked_add(Duration::new(ttl, 0))
            .ok_or_else(|| anyhow!("Duration overflow."))?
            .duration_since(UNIX_EPOCH)
            .map_err(|e| anyhow!(e.to_string()))?
            .as_secs();
        Ok(Self { cid, expires })
    }

    /// Creates a new [NSRecord] from serialized bytes. See [NSRecord]
    /// for serialization details.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, Error> {
        let string = String::from_utf8(bytes).map_err(|e| anyhow!(e.to_string()))?;
        serde_json::from_str(&string).map_err(|e| anyhow!(e.to_string()))
    }

    /// Serializes the record into bytes. See [NSRecord] for
    /// serialization details.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let bytes = serde_json::to_vec(self).map_err(|e| anyhow!(e.to_string()))?;
        Ok(bytes)
    }

    /// Validates the [NSRecord] based off of its expiration time
    /// compared to the current system time.
    pub fn is_expired(&self) -> bool {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs() >= self.expires,
            Err(_) => false,
        }
    }
}

impl fmt::Display for NSRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match serde_json::to_string(self) {
            Ok(record_str) => write!(f, "{}", record_str),
            Err(_) => write!(f, "{{ INVALID_RECORD }}"),
        }
    }
}

/// Serialization for NSRecords. While [Cid] has built-in serde
/// support under a feature flag, we roll our own to store the Cid
/// as a string rather than bytes.
impl Serialize for NSRecord {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("NSRecord", 2)?;
        state.serialize_field("cid", &self.cid.to_string())?;
        state.serialize_field("exp", &self.expires)?;
        state.end()
    }
}

/// Deserialization for NSRecords. While [Cid] has built-in serde
/// support under a feature flag, we roll our own to store the Cid
/// as a string rather than bytes.
/// For more details on custom deserialization: <https://serde.rs/deserialize-struct.html>
impl<'de> Deserialize<'de> for NSRecord {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        const FIELDS: &[&str] = &["cid", "exp"];
        enum Field {
            Cid,
            Expires,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`cid` or `exp`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "cid" => Ok(Field::Cid),
                            "exp" => Ok(Field::Expires),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct NSRecordVisitor;

        impl<'de> Visitor<'de> for NSRecordVisitor {
            type Value = NSRecord;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct NSRecord")
            }

            // This handler is not used with serde_json, but for sequence-based
            // deserializers e.g. postcard
            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let cid = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let expires = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(NSRecord::new(cid, expires))
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut cid = None;
                let mut expires = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Cid => {
                            if cid.is_some() {
                                return Err(de::Error::duplicate_field("cid"));
                            }
                            cid = Some(
                                Cid::from_str(map.next_value::<&str>()?)
                                    .map_err(de::Error::custom)?,
                            );
                        }
                        Field::Expires => {
                            if expires.is_some() {
                                return Err(de::Error::duplicate_field("exp"));
                            }
                            expires = Some(map.next_value()?);
                        }
                    }
                }
                let cid = cid.ok_or_else(|| de::Error::missing_field("cid"))?;
                let expires = expires.ok_or_else(|| de::Error::missing_field("exp"))?;
                Ok(NSRecord::new(cid, expires))
            }
        }

        deserializer.deserialize_struct("NSRecord", FIELDS, NSRecordVisitor)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cid::multihash::{Code, MultihashDigest};
    use std::str::FromStr;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn new_cid(s: &[u8]) -> Cid {
        Cid::new_v1(0x55, Code::Sha2_256.digest(s))
    }

    #[test]
    fn test_nsrecord_new() -> Result<(), Box<dyn std::error::Error>> {
        let cid = new_cid(b"foo");
        let expires: u64 = SystemTime::now()
            .checked_add(Duration::new(3600, 0))
            .expect("valid duration")
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        let record = NSRecord::new(cid, expires);
        assert_eq!(record.cid, cid, "NSRecord::new() cid works");
        assert_eq!(record.expires, expires, "NSRecord::new() expires works");
        Ok(())
    }

    #[test]
    fn test_nsrecord_new_from_ttl() -> Result<(), Box<dyn std::error::Error>> {
        let cid = new_cid(b"foo");
        let ttl = 3600;
        let expected_expiration: u64 = SystemTime::now()
            .checked_add(Duration::new(ttl, 0))
            .expect("valid duration")
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        let record = NSRecord::new_from_ttl(cid, ttl)?;
        assert_eq!(record.cid, cid);
        assert!(record.expires.abs_diff(expected_expiration) < 5);
        Ok(())
    }

    #[test]
    fn test_nsrecord_from_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let record = NSRecord::from_bytes(
            String::from(
                r#"{
                  "cid": "bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy",
                  "exp": 1667262626
                }"#,
            )
            .into_bytes(),
        )?;

        assert_eq!(
            record.cid.to_string(),
            "bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy"
        );
        assert_eq!(record.expires, 1667262626);
        Ok(())
    }

    #[test]
    fn test_nsrecord_to_bytes() -> Result<(), Box<dyn std::error::Error>> {
        let cid_str = "bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy";
        let record = NSRecord::new(Cid::from_str(cid_str)?, 1667262626);
        let bytes = record.to_bytes()?;

        assert_eq!(&String::from_utf8(bytes.clone())?, "{\"cid\":\"bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy\",\"exp\":1667262626}");
        let de_record = NSRecord::from_bytes(bytes)?;
        assert_eq!(de_record.cid.to_string(), cid_str);
        assert_eq!(de_record.expires, 1667262626);
        Ok(())
    }

    #[test]
    fn test_nsrecord_is_expired() -> Result<(), Box<dyn std::error::Error>> {
        let record = NSRecord::new_from_ttl(new_cid(b"foo"), 3600)?;
        assert!(!record.is_expired());

        let record = NSRecord::new(
            new_cid(b"foo"),
            60 * 60 * 24 * 365, /* a year after unix epoch */
        );
        assert!(record.is_expired());
        Ok(())
    }

    #[test]
    fn test_nsrecord_to_string() -> Result<(), Box<dyn std::error::Error>> {
        let cid_str = "bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy";
        let record = NSRecord::new(Cid::from_str(cid_str)?, 1667262626);
        assert_eq!(record.to_string(), "{\"cid\":\"bafkreibme22gw2h7y2h7tg2fhqotaqjucnbc24deqo72b6mkl2egezxhvy\",\"exp\":1667262626}");
        Ok(())
    }
}
