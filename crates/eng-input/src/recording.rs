use eng_core::{PlayerCommand, Tick};
use eng_schema_core::container::{read_container, write_container};
use eng_schema_core::{DecodeError, Schema};

pub const RECORDING_KIND: [u8; 4] = *b"RECD";
pub const RECORDING_VERSION: u32 = 1;

/// Who and what produced a recording (spec 6.4). In Level 1 a recording is
/// valid only for the exact build in its header.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
#[schema(name = "eng.input.RecordingHeader", version = 1)]
pub struct RecordingHeader {
    pub game: String,
    pub engine_sha: String,
    pub game_sha: String,
    pub rustc: String,
    pub sim_protocol: u64,
    /// Content hash of the game's sim data; 0 until levels exist (rung 3).
    pub sim_data_hash: u64,
    pub seed: u64,
    pub players: u8,
    pub tick_rate: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Schema)]
#[schema(name = "eng.input.TickHash", version = 1)]
pub struct TickHash {
    pub tick: Tick,
    pub hash: u64,
}

/// A session's applied commands plus the state hashes they produced.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
#[schema(name = "eng.input.Recording", version = 1)]
pub struct Recording {
    pub header: RecordingHeader,
    /// Tick-major: ticks `1..=N`, `players` commands each.
    pub commands: Vec<PlayerCommand>,
    /// The state hash at tick 0 and at the ticks the recorder chose.
    pub hashes: Vec<TickHash>,
}

impl Recording {
    /// The number of ticks recorded.
    pub fn ticks(&self) -> u64 {
        (self.commands.len() / usize::from(self.header.players.max(1))) as u64
    }

    /// The commands applied at `tick` (`1..=ticks()`).
    pub fn commands_at(&self, tick: Tick) -> &[PlayerCommand] {
        let players = usize::from(self.header.players);
        let start = (tick.0 as usize - 1) * players;
        &self.commands[start..start + players]
    }

    /// Checks the structure: player count, tick stamps, hash order.
    pub fn validate(&self) -> Result<(), String> {
        let players = usize::from(self.header.players);
        if players == 0 {
            return Err("header says 0 players".into());
        }
        if !self.commands.len().is_multiple_of(players) {
            return Err(format!(
                "{} commands do not divide into {players} players",
                self.commands.len()
            ));
        }
        for (i, cmd) in self.commands.iter().enumerate() {
            let want = Tick((i / players) as u64 + 1);
            if cmd.tick != want {
                return Err(format!("command {i} is stamped {} not {want}", cmd.tick));
            }
        }
        let ticks = self.ticks();
        // Without hashes for tick 0 and the last tick, verifying proves nothing.
        if self.hashes.first().map(|h| h.tick) != Some(Tick::ZERO) {
            return Err("no state hash for tick 0".into());
        }
        if self.hashes.last().map(|h| h.tick) != Some(Tick(ticks)) {
            return Err(format!("no state hash for the last tick ({ticks})"));
        }
        let mut last = None;
        for h in &self.hashes {
            if h.tick.0 > ticks || last.is_some_and(|l| h.tick <= l) {
                return Err(format!("hash for {} is out of order or range", h.tick));
            }
            last = Some(h.tick);
        }
        Ok(())
    }
}

pub fn write_recording(recording: &Recording) -> Vec<u8> {
    write_container(RECORDING_KIND, RECORDING_VERSION, recording)
}

pub fn read_recording(bytes: &[u8]) -> Result<Recording, DecodeError> {
    read_container(bytes, RECORDING_KIND, RECORDING_VERSION)
}
