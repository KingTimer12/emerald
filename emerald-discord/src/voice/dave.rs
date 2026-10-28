//! DAVE, Discord's end-to-end encryption for calls (protocol version 1,
//! through `davey`): an MLS group of the people in the call, whose keys
//! seal each Opus frame before the transport cipher does. Discord's voice
//! server is the group's external sender; it proposes adds and removes,
//! a member commits them, and newcomers get a welcome.

use std::borrow::Cow;
use std::num::NonZeroU16;

use anyhow::{Result, anyhow};
use davey::{DaveSession, MediaType, ProposalsOperationType};

use crate::packets::voice as packets;

pub struct Dave {
    session: DaveSession,
    user: u64,
    channel: u64,
    /// The version the voice server chose; 0 means the call is not
    /// end-to-end encrypted and frames pass through.
    version: u16,
    /// Who the voice server says is connected, us included. Proposals
    /// may only add these.
    members: Vec<u64>,
}

impl Dave {
    pub fn new(user: u64, channel: u64) -> Result<Self> {
        let session = DaveSession::new(version(), user, channel, None).map_err(|e| anyhow!("DAVE setup: {e}"))?;
        Ok(Self { session, user, channel, version: 0, members: vec![user] })
    }

    /// The session description named the version to speak.
    pub fn set_version(&mut self, version: u16) {
        self.version = version;
    }

    pub fn encrypted(&self) -> bool {
        self.version > 0
    }

    /// Frames can go out, and come in, now.
    pub fn ready(&self) -> bool {
        !self.encrypted() || self.session.is_ready()
    }

    /// The code people compare to check nobody listens in.
    pub fn privacy_code(&self) -> Option<String> {
        self.session.voice_privacy_code().map(str::to_owned)
    }

    pub fn connect(&mut self, users: impl IntoIterator<Item = u64>) {
        for user in users {
            if !self.members.contains(&user) {
                self.members.push(user);
            }
        }
    }

    pub fn disconnect(&mut self, user: u64) {
        if user != self.user {
            self.members.retain(|u| *u != user);
        }
    }

    /// Binary op 26, sent once the session is described and again after
    /// each reset.
    pub fn key_package(&mut self) -> Result<Vec<u8>> {
        let package = self.session.create_key_package().map_err(|e| anyhow!("DAVE key package: {e}"))?;
        Ok(packets::key_package(&package))
    }

    /// Op 25.
    pub fn external_sender(&mut self, data: &[u8]) -> Result<()> {
        self.session.set_external_sender(data).map_err(|e| anyhow!("DAVE external sender: {e}"))
    }

    /// Op 27; the commit to send back, if ours is needed.
    pub fn proposals(&mut self, data: &[u8]) -> Result<Option<Vec<u8>>> {
        let (&operation, proposals) = data.split_first().ok_or_else(|| anyhow!("empty DAVE proposals"))?;
        let operation = match operation {
            0 => ProposalsOperationType::APPEND,
            1 => ProposalsOperationType::REVOKE,
            other => return Err(anyhow!("unknown DAVE proposal operation {other}")),
        };
        // Before the external sender arrives there is no group to add to.
        if self.session.group().is_none() {
            return Ok(None);
        }
        let commit = self
            .session
            .process_proposals(operation, proposals, Some(&self.members))
            .map_err(|e| anyhow!("DAVE proposals: {e}"))?;
        Ok(commit.map(|c| packets::commit_welcome(&c.commit, c.welcome.as_deref())))
    }

    /// Ops 29 and 30: a commit announced to all, or our welcome into the
    /// group. Returns the transition they belong to, to acknowledge or
    /// report as invalid, and whether it worked.
    pub fn transition(&mut self, opcode: u8, data: &[u8]) -> (u16, Result<()>) {
        let Some((id, message)) = data.split_first_chunk::<2>() else {
            return (0, Err(anyhow!("short DAVE transition")));
        };
        let processed = if opcode == packets::op::DAVE_ANNOUNCE_COMMIT {
            self.session.process_commit(message).map_err(|e| anyhow!("DAVE commit: {e}"))
        } else {
            self.session.process_welcome(message).map_err(|e| anyhow!("DAVE welcome: {e}"))
        };
        (u16::from_be_bytes(*id), processed)
    }

    /// Starts over with a fresh group, after a failed transition or when
    /// the voice server opens a new epoch.
    pub fn reset(&mut self) -> Result<()> {
        self.session.reinit(version(), self.user, self.channel, None).map_err(|e| anyhow!("DAVE reset: {e}"))
    }

    /// Our Opus frame, sealed for the group.
    pub fn encrypt<'a>(&mut self, opus: &'a [u8]) -> Option<Cow<'a, [u8]>> {
        if !self.encrypted() {
            return Some(Cow::Borrowed(opus));
        }
        self.session.encrypt_opus(opus).ok()
    }

    /// Someone's frame, opened; `None` until their keys are known.
    pub fn decrypt(&mut self, user: u64, frame: Vec<u8>) -> Option<Vec<u8>> {
        if !self.encrypted() {
            return Some(frame);
        }
        // Silence frames are never sealed.
        if frame == davey::OPUS_SILENCE_PACKET {
            return Some(frame);
        }
        self.session.decrypt(user, MediaType::AUDIO, &frame).ok()
    }
}

fn version() -> NonZeroU16 {
    NonZeroU16::new(u16::from(packets::DAVE_VERSION)).expect("DAVE version is not zero")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unencrypted_calls_pass_frames_through() {
        let mut dave = Dave::new(1, 2).unwrap();
        assert!(dave.ready());
        assert_eq!(dave.encrypt(b"opus").as_deref(), Some(&b"opus"[..]));
        assert_eq!(dave.decrypt(3, b"opus".to_vec()), Some(b"opus".to_vec()));
    }

    #[test]
    fn encrypted_calls_wait_for_the_group() {
        let mut dave = Dave::new(1, 2).unwrap();
        dave.set_version(1);
        assert!(!dave.ready());
        assert!(dave.encrypt(b"opus").is_none());
        assert_eq!(dave.key_package().unwrap()[0], packets::op::DAVE_KEY_PACKAGE);
        // Proposals before the external sender are ignored.
        assert!(dave.proposals(&[0]).unwrap().is_none());
        assert!(dave.transition(packets::op::DAVE_ANNOUNCE_COMMIT, &[0]).1.is_err());
    }
}
