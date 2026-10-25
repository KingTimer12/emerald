//! The server list as the user arranged it: folders and order, synced from
//! their settings, and which folders are open here.

use std::collections::HashSet;

use emerald_discord::model::{GuildFolder, Snowflake};

/// One item of the server list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Guild(Snowflake),
    /// The folder at this index of [`ServerList::folder`].
    Folder(usize),
}

#[derive(Default)]
pub struct ServerList {
    folders: Vec<GuildFolder>,
    /// Folders opened here, by id; Discord keeps this per device.
    open: HashSet<i64>,
}

impl ServerList {
    /// The folders from the user's settings; `None` keeps the current ones.
    pub fn set(&mut self, folders: Option<Vec<GuildFolder>>) {
        if let Some(folders) = folders {
            self.folders = folders;
        }
    }

    pub fn folder(&self, ix: usize) -> Option<&GuildFolder> {
        self.folders.get(ix)
    }

    pub fn is_open(&self, ix: usize) -> bool {
        self.folders.get(ix).and_then(|f| f.id).is_some_and(|id| self.open.contains(&id))
    }

    pub fn toggle(&mut self, ix: usize) {
        let Some(id) = self.folders.get(ix).and_then(|f| f.id) else { return };
        if !self.open.remove(&id) {
            self.open.insert(id);
        }
    }

    /// The folder holding `guild`, if a real folder does.
    pub fn folder_of(&self, guild: Snowflake) -> Option<usize> {
        self.folders.iter().position(|f| f.is_folder() && f.guilds.contains(&guild))
    }

    /// The list for the guilds the user is in (`present`, in their own
    /// order): those the settings do not place yet first, as Discord shows
    /// new servers on top, then the folders and servers in their order.
    /// Folders with none of their servers left are skipped.
    pub fn items(&self, present: &[Snowflake]) -> Vec<Item> {
        let placed: HashSet<Snowflake> = self.folders.iter().flat_map(|f| f.guilds.iter().copied()).collect();
        let mut items: Vec<Item> = present
            .iter()
            .filter(|g| !placed.contains(g))
            .map(|g| Item::Guild(*g))
            .collect();
        for (ix, folder) in self.folders.iter().enumerate() {
            let here = || folder.guilds.iter().filter(|g| present.contains(g));
            if folder.is_folder() {
                if here().next().is_some() {
                    items.push(Item::Folder(ix));
                }
            } else {
                items.extend(here().map(|g| Item::Guild(*g)));
            }
        }
        items
    }

    /// Every guild in list order, folders flattened: the order spaces are
    /// kept in.
    pub fn order(&self, present: &[Snowflake]) -> Vec<Snowflake> {
        self.items(present)
            .into_iter()
            .flat_map(|item| match item {
                Item::Guild(g) => vec![g],
                Item::Folder(ix) => self.folders[ix].guilds.iter().filter(|g| present.contains(g)).copied().collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(id: Option<i64>, guilds: &[u64]) -> GuildFolder {
        GuildFolder {
            id,
            guilds: guilds.iter().map(|g| Snowflake(*g)).collect(),
            ..GuildFolder::default()
        }
    }

    #[test]
    fn new_servers_go_first_and_folders_keep_their_place() {
        let mut list = ServerList::default();
        list.set(Some(vec![folder(None, &[1]), folder(Some(7), &[2, 3]), folder(None, &[4]), folder(Some(8), &[99])]));
        let present: Vec<Snowflake> = [1, 2, 3, 4, 5].map(Snowflake).to_vec();
        assert_eq!(
            list.items(&present),
            [Item::Guild(Snowflake(5)), Item::Guild(Snowflake(1)), Item::Folder(1), Item::Guild(Snowflake(4))]
        );
        assert_eq!(list.order(&present), [5, 1, 2, 3, 4].map(Snowflake));
        assert_eq!(list.folder_of(Snowflake(3)), Some(1));
        assert_eq!(list.folder_of(Snowflake(1)), None);
        assert!(!list.is_open(1));
        list.toggle(1);
        assert!(list.is_open(1));
        // A partial update without folders changes nothing.
        list.set(None);
        assert_eq!(list.folder(1).and_then(|f| f.id), Some(7));
    }
}
