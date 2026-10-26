//! The server list's order and folders, synced across devices inside the
//! user's settings protobuf (`PreloadedUserSettings.guild_folders`). Only
//! that field is read, with a protobuf reader just big enough for it.

use super::Snowflake;
use super::proto::{self, Reader, Value, wrapped_bytes, wrapped_varint};

/// A folder of servers, or a lone server when it has no `id`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuildFolder {
    pub id: Option<i64>,
    pub name: Option<String>,
    /// RGB, when the user picked one.
    pub color: Option<u32>,
    pub guilds: Vec<Snowflake>,
}

impl GuildFolder {
    /// A real folder, drawn as one; entries without an id are single
    /// servers in their place in the list.
    pub fn is_folder(&self) -> bool {
        self.id.is_some()
    }
}

/// `PreloadedUserSettings` field numbers, as the web client's protobuf
/// definitions give them.
mod field {
    pub const GUILD_FOLDERS: u32 = 14;
    pub const FOLDERS: u32 = 1;
    pub const FOLDER_GUILD_IDS: u32 = 1;
    pub const FOLDER_ID: u32 = 2;
    pub const FOLDER_NAME: u32 = 3;
    pub const FOLDER_COLOR: u32 = 4;
}

/// The folders in `proto`, base64 as READY and USER_SETTINGS_PROTO_UPDATE
/// send it. `None` when it is malformed or holds no folder field (a
/// partial update that left them alone).
pub fn guild_folders(proto: &str) -> Option<Vec<GuildFolder>> {
    let bytes = proto::decode_base64(proto)?;
    let folders = proto::message(&bytes, field::GUILD_FOLDERS)?;
    let mut list = Vec::new();
    for (number, value) in Reader::new(folders).fields().ok()? {
        if let (field::FOLDERS, Value::Bytes(folder)) = (number, value) {
            list.push(folder_from(folder)?);
        }
    }
    Some(list)
}

fn folder_from(bytes: &[u8]) -> Option<GuildFolder> {
    let mut folder = GuildFolder::default();
    for (number, value) in Reader::new(bytes).fields().ok()? {
        match (number, value) {
            (field::FOLDER_GUILD_IDS, Value::Fixed64(id)) => folder.guilds.push(Snowflake(id)),
            // Packed, as protobuf writes repeated scalars.
            (field::FOLDER_GUILD_IDS, Value::Bytes(packed)) => {
                if packed.len() % 8 != 0 {
                    return None;
                }
                folder.guilds.extend(packed.as_chunks::<8>().0.iter().map(|id| Snowflake(u64::from_le_bytes(*id))));
            }
            (field::FOLDER_ID, Value::Bytes(wrapper)) => folder.id = Some(wrapped_varint(wrapper)? as i64),
            (field::FOLDER_NAME, Value::Bytes(wrapper)) => {
                let name = wrapped_bytes(wrapper)?;
                folder.name = Some(String::from_utf8_lossy(name).into_owned());
            }
            (field::FOLDER_COLOR, Value::Bytes(wrapper)) => folder.color = Some(wrapped_varint(wrapper)? as u32),
            _ => {}
        }
    }
    Some(folder)
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;

    fn varint(mut value: u64, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }

    fn bytes_field(number: u32, bytes: &[u8], out: &mut Vec<u8>) {
        varint(u64::from(number) << 3 | 2, out);
        varint(bytes.len() as u64, out);
        out.extend_from_slice(bytes);
    }

    fn wrapper_varint(value: u64) -> Vec<u8> {
        let mut out = vec![0x08];
        varint(value, &mut out);
        out
    }

    #[test]
    fn folders_come_out_of_the_settings_proto() {
        let mut folder = Vec::new();
        let ids: Vec<u8> = [11u64, 22].iter().flat_map(|id| id.to_le_bytes()).collect();
        bytes_field(1, &ids, &mut folder);
        bytes_field(2, &wrapper_varint(777), &mut folder);
        let mut name = Vec::new();
        bytes_field(1, b"Games", &mut name);
        bytes_field(3, &name, &mut folder);
        bytes_field(4, &wrapper_varint(0x5865f2), &mut folder);
        let mut lone = Vec::new();
        bytes_field(1, &33u64.to_le_bytes(), &mut lone);

        let mut folders = Vec::new();
        bytes_field(1, &folder, &mut folders);
        bytes_field(1, &lone, &mut folders);
        let mut settings = Vec::new();
        // A field before ours, to skip: status (11) as a varint.
        varint(11 << 3, &mut settings);
        varint(1, &mut settings);
        bytes_field(field::GUILD_FOLDERS, &folders, &mut settings);
        let encoded = base64::engine::general_purpose::STANDARD.encode(settings);

        let list = guild_folders(&encoded).unwrap();
        assert_eq!(
            list[0],
            GuildFolder {
                id: Some(777),
                name: Some("Games".into()),
                color: Some(0x5865f2),
                guilds: vec![Snowflake(11), Snowflake(22)],
            }
        );
        assert!(!list[1].is_folder());
        assert_eq!(list[1].guilds, [Snowflake(33)]);
        assert_eq!(guild_folders("not base64!"), None);
        assert_eq!(guild_folders(""), None);
    }
}
