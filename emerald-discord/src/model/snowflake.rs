use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Snowflake(pub u64);

/// Discord epoch (2015-01-01T00:00:00Z) in Unix milliseconds.
pub const DISCORD_EPOCH_MS: u64 = 1_420_070_400_000;

impl Snowflake {
    /// Creation time encoded in the id, in Unix seconds.
    pub fn unix_secs(self) -> i64 {
        (((self.0 >> 22) + DISCORD_EPOCH_MS) / 1000) as i64
    }
}

/// Rate limits and the microcache scope requests by the bare id.
impl From<Snowflake> for u64 {
    fn from(id: Snowflake) -> Self {
        id.0
    }
}

impl fmt::Display for Snowflake {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// As a decimal string, the way Discord writes ids.
impl Serialize for Snowflake {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Snowflake {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(Decimal).map(Snowflake)
    }
}

/// A `u64` sent as a decimal string or a number, as Discord does for ids
/// and permission bitfields.
pub(super) struct Decimal;

impl Visitor<'_> for Decimal {
    type Value = u64;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a decimal u64 as a string or number")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<u64, E> {
        v.parse().map_err(E::custom)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<u64, E> {
        Ok(v)
    }
}

/// Permission bitfields arrive as decimal strings.
pub(super) fn bits<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    d.deserialize_any(Decimal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snowflake_time() {
        // Discord's docs example: 175928847299117063 is 2016-04-30T11:18:25.796Z.
        assert_eq!(Snowflake(175928847299117063).unix_secs(), 1462015105);
    }

    #[test]
    fn ids_parse_from_strings_and_numbers() {
        assert_eq!(serde_json::from_str::<Snowflake>(r#""42""#).unwrap(), Snowflake(42));
        assert_eq!(serde_json::from_str::<Snowflake>("42").unwrap(), Snowflake(42));
    }
}
