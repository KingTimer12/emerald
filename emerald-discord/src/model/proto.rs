//! A protobuf reader and writer just big enough for the user's settings
//! (`PreloadedUserSettings`), which READY, USER_SETTINGS_PROTO_UPDATE and
//! the settings PATCH carry as base64. Field numbers come from the web
//! client's protobuf definitions; each reader picks out only its fields.

use base64::Engine;

/// One field's value, by wire type.
pub(crate) enum Value<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}

/// The `value` of the `google.protobuf` wrapper types.
pub(crate) const WRAPPED: u32 = 1;

/// `proto` as base64 decodes it; `None` when it is not base64.
pub(crate) fn decode_base64(proto: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(proto).ok()
}

pub(crate) fn encode_base64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    /// Every field of the message, in order.
    pub(crate) fn fields(mut self) -> Result<Vec<(u32, Value<'a>)>, ()> {
        let mut fields = Vec::new();
        while self.at < self.bytes.len() {
            let key = self.varint()?;
            let number = u32::try_from(key >> 3).map_err(drop)?;
            let value = match key & 7 {
                0 => Value::Varint(self.varint()?),
                1 => Value::Fixed64(u64::from_le_bytes(self.take(8)?.try_into().map_err(drop)?)),
                2 => {
                    let len = usize::try_from(self.varint()?).map_err(drop)?;
                    Value::Bytes(self.take(len)?)
                }
                5 => Value::Fixed32(u32::from_le_bytes(self.take(4)?.try_into().map_err(drop)?)),
                // Groups are long gone from protobuf; nothing here uses them.
                _ => return Err(()),
            };
            fields.push((number, value));
        }
        Ok(fields)
    }

    fn varint(&mut self) -> Result<u64, ()> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = *self.bytes.get(self.at).ok_or(())?;
            self.at += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(())
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], ()> {
        let end = self.at.checked_add(len).filter(|end| *end <= self.bytes.len()).ok_or(())?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }
}

/// The last message field `number` of `bytes`, which is what protobuf
/// keeps when one appears twice.
pub(crate) fn message(bytes: &[u8], number: u32) -> Option<&[u8]> {
    Reader::new(bytes).fields().ok()?.into_iter().rev().find_map(|(n, value)| match value {
        Value::Bytes(inner) if n == number => Some(inner),
        _ => None,
    })
}

/// The value of an `Int64Value`, `UInt32Value` or `BoolValue`; zero when
/// left out, as protobuf omits defaults.
pub(crate) fn wrapped_varint(bytes: &[u8]) -> Option<u64> {
    let fields = Reader::new(bytes).fields().ok()?;
    Some(
        fields
            .into_iter()
            .find_map(|(number, value)| match (number, value) {
                (WRAPPED, Value::Varint(v)) => Some(v),
                _ => None,
            })
            .unwrap_or(0),
    )
}

/// The value of a `StringValue` or `BytesValue`; empty when left out.
pub(crate) fn wrapped_bytes(bytes: &[u8]) -> Option<&[u8]> {
    let fields = Reader::new(bytes).fields().ok()?;
    Some(
        fields
            .into_iter()
            .find_map(|(number, value)| match (number, value) {
                (WRAPPED, Value::Bytes(v)) => Some(v),
                _ => None,
            })
            .unwrap_or_default(),
    )
}

/// Builds a message field by field. Fields left unwritten stay unchanged
/// when the message is a partial settings update.
#[derive(Default)]
pub(crate) struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    pub(crate) fn varint(&mut self, number: u32, value: u64) -> &mut Self {
        self.key(number, 0);
        self.raw_varint(value);
        self
    }

    pub(crate) fn fixed64(&mut self, number: u32, value: u64) -> &mut Self {
        self.key(number, 1);
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub(crate) fn bytes(&mut self, number: u32, value: &[u8]) -> &mut Self {
        self.key(number, 2);
        self.raw_varint(value.len() as u64);
        self.bytes.extend_from_slice(value);
        self
    }

    /// A repeated `fixed64`, packed as protobuf 3 writes it.
    pub(crate) fn packed_fixed64(&mut self, number: u32, values: &[u64]) -> &mut Self {
        let packed: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        self.bytes(number, &packed)
    }

    pub(crate) fn message(&mut self, number: u32, inner: Writer) -> &mut Self {
        self.bytes(number, &inner.bytes)
    }

    /// A `BoolValue` or `UInt32Value`. A zero value is still written, as a
    /// wrapper with no field, so it reads as set rather than left out.
    pub(crate) fn wrapped_varint(&mut self, number: u32, value: u64) -> &mut Self {
        let mut inner = Writer::default();
        if value != 0 {
            inner.varint(WRAPPED, value);
        }
        self.message(number, inner)
    }

    pub(crate) fn wrapped_string(&mut self, number: u32, value: &str) -> &mut Self {
        let mut inner = Writer::default();
        if !value.is_empty() {
            inner.bytes(WRAPPED, value.as_bytes());
        }
        self.message(number, inner)
    }

    /// `section`'s fields as they were, but those numbered in `replaced`,
    /// which the caller writes anew. Fields this reader does not know of
    /// are kept, so nothing set elsewhere is lost.
    pub(crate) fn keep(section: &[u8], replaced: &[u32]) -> Option<Self> {
        let mut writer = Writer::default();
        for (number, value) in Reader::new(section).fields().ok()? {
            if replaced.contains(&number) {
                continue;
            }
            match value {
                Value::Varint(v) => writer.varint(number, v),
                Value::Fixed64(v) => writer.fixed64(number, v),
                Value::Bytes(b) => writer.bytes(number, b),
                Value::Fixed32(v) => {
                    writer.key(number, 5);
                    writer.bytes.extend_from_slice(&v.to_le_bytes());
                    &mut writer
                }
            };
        }
        Some(writer)
    }

    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    fn key(&mut self, number: u32, wire: u8) {
        self.raw_varint(u64::from(number) << 3 | u64::from(wire));
    }

    fn raw_varint(&mut self, mut value: u64) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                self.bytes.push(byte);
                return;
            }
            self.bytes.push(byte | 0x80);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_fields_read_back() {
        let mut inner = Writer::default();
        inner.wrapped_varint(4, 300).wrapped_string(5, "hi");
        let mut outer = Writer::default();
        outer.varint(1, 7).fixed64(2, 99).packed_fixed64(3, &[1, 2]).message(9, inner);
        let bytes = outer.into_bytes();

        let fields = Reader::new(&bytes).fields().unwrap();
        assert!(matches!(fields[0], (1, Value::Varint(7))));
        assert!(matches!(fields[1], (2, Value::Fixed64(99))));
        assert!(matches!(fields[2], (3, Value::Bytes(b)) if b.len() == 16));
        let inner = message(&bytes, 9).unwrap();
        assert_eq!(wrapped_varint(message(inner, 4).unwrap()), Some(300));
        assert_eq!(wrapped_bytes(message(inner, 5).unwrap()), Some(&b"hi"[..]));
    }

    #[test]
    fn kept_sections_lose_only_replaced_fields() {
        let mut section = Writer::default();
        section.varint(1, 5).bytes(2, b"old").fixed64(3, 9);
        let mut kept = Writer::keep(&section.into_bytes(), &[2]).unwrap();
        kept.bytes(2, b"new");
        let fields = Reader::new(&kept.bytes).fields().unwrap();
        assert!(matches!(fields[0], (1, Value::Varint(5))));
        assert!(matches!(fields[1], (3, Value::Fixed64(9))));
        assert!(matches!(fields[2], (2, Value::Bytes(b"new"))));
    }

    #[test]
    fn false_and_zero_are_still_sent() {
        let mut writer = Writer::default();
        writer.wrapped_varint(4, 0).wrapped_varint(17, 1);
        assert_eq!(writer.into_bytes(), [4 << 3 | 2, 0, 0x8a, 0x01, 2, 0x08, 1]);
    }
}
