//! Versioned, ROM-pinned cooperative training tasks with explicit completion rules.
use crate::{
    Result, adapter, base, bubble, chip, dd2, ice, jackal, lifeforce, rcr, snow, tmnt, worldcup,
};
use nes_core::machine::Machine;
use serde::Serialize;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Task {
    Base1,
    SnowFloor1,
    BubbleRound1,
    IceMountain1,
    RiverCityMoose,
    TmntScene1,
    DoubleDragonMission1,
    WorldCupFirstMatch,
    ChipZone0,
    JackalStage1,
    LifeForceStage1,
}
impl Task {
    pub fn parse(name: &str) -> Result<Self> {
        match name {
            "lifeforce-stage1" => Ok(Self::LifeForceStage1),
            "jackal-stage1" => Ok(Self::JackalStage1),
            "chipndale-zone0" => Ok(Self::ChipZone0),
            "worldcup-firstmatch" => Ok(Self::WorldCupFirstMatch),
            "doubledragon2-mission1" => Ok(Self::DoubleDragonMission1),
            "tmnt2-scene1" => Ok(Self::TmntScene1),
            "rivercity-moose" => Ok(Self::RiverCityMoose),
            "contra-base1" => Ok(Self::Base1),
            "snowbros-floor1" => Ok(Self::SnowFloor1),
            "iceclimber-mountain1" => Ok(Self::IceMountain1),
            "bubblebobble-round1" => Ok(Self::BubbleRound1),
            _ => Err("unknown research task".into()),
        }
    }
    pub fn schema(self) -> &'static str {
        match self {
            Self::LifeForceStage1 => lifeforce::SCHEMA,
            Self::JackalStage1 => jackal::SCHEMA,
            Self::ChipZone0 => chip::SCHEMA,
            Self::WorldCupFirstMatch => worldcup::SCHEMA,
            Self::DoubleDragonMission1 => dd2::SCHEMA,
            Self::TmntScene1 => tmnt::SCHEMA,
            Self::RiverCityMoose => rcr::SCHEMA,
            Self::Base1 => base::SCHEMA,
            Self::SnowFloor1 => snow::SCHEMA,
            Self::BubbleRound1 => bubble::SCHEMA,
            Self::IceMountain1 => ice::SCHEMA,
        }
    }
    pub fn validate(self, m: &Machine) -> Result<()> {
        let hash = match self {
            Self::LifeForceStage1 => lifeforce::ROM_SHA256,
            Self::JackalStage1 => jackal::ROM_SHA256,
            Self::ChipZone0 => chip::ROM_SHA256,
            Self::WorldCupFirstMatch => worldcup::ROM_SHA256,
            Self::DoubleDragonMission1 => dd2::ROM_SHA256,
            Self::TmntScene1 => tmnt::ROM_SHA256,
            Self::RiverCityMoose => rcr::ROM_SHA256,
            Self::Base1 => adapter::ROM_SHA256,
            Self::SnowFloor1 => snow::ROM_SHA256,
            Self::BubbleRound1 => bubble::ROM_SHA256,
            Self::IceMountain1 => ice::ROM_SHA256,
        };
        if m.cartridge_hash() != hash {
            return Err("task requires its pinned ROM revision".into());
        }
        Ok(())
    }
    pub fn initialize(self, machine: &mut Machine, start: Option<&[u8]>) -> Result<()> {
        self.validate(machine)?;
        match self {
            Self::LifeForceStage1 => lifeforce::initialize(machine, start)?,
            Self::JackalStage1 => jackal::initialize(machine, start)?,
            Self::ChipZone0 => chip::initialize(machine, start)?,
            Self::WorldCupFirstMatch => worldcup::initialize(machine, start)?,
            Self::DoubleDragonMission1 => dd2::initialize(machine, start)?,
            Self::TmntScene1 => tmnt::initialize(machine, start)?,
            Self::RiverCityMoose => rcr::initialize(machine, start)?,
            Self::Base1 => base::initialize(machine, start)?,
            Self::IceMountain1 => ice::initialize(machine, start)?,
            Self::BubbleRound1 => bubble::initialize(machine, start)?,
            Self::SnowFloor1 => snow::initialize(machine, start)?,
        }
        let state = self.read(machine)?;
        if !state.gameplay || state.active != [true, true] || state.dead != [false, false] {
            return Err("reset must reach live two-player task gameplay".into());
        }
        Ok(())
    }
    /// One-player reset through the cartridge menu. Never substitutes an idle P2.
    pub fn initialize_solo(self, machine: &mut Machine) -> Result<()> {
        self.validate(machine)?;
        let definition = crate::registry::schema(self.schema()).ok_or("unknown task")?;
        if definition.solo_reset.is_none() {
            return Err("task has no verified solo reset".into());
        }
        for &(frames, masks) in definition.solo_menu {
            crate::registry::run_reset(machine, masks, frames)?;
        }
        // Some titles still load the level after the final menu confirmation.
        // Wait with neutral ports, bounded, and stop at the first playable frame.
        for _ in 0..1200 {
            if self.read(machine)?.gameplay {
                break;
            }
            crate::registry::run_reset(machine, [0, 0], 1)?;
        }
        let state = self.read(machine)?;
        if !state.gameplay || state.active != [true, false] || state.dead[0] {
            return Err("solo reset must reach live P1 with P2 absent".into());
        }
        Ok(())
    }
    pub fn read(self, m: &Machine) -> Result<State> {
        self.validate(m)?;
        match self {
            Self::LifeForceStage1 => {
                let f = lifeforce::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.stage_cleared,
                    progress: f.progress(),
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
            Self::JackalStage1 => {
                let f = jackal::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.stage_cleared,
                    progress: f.progress(),
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
            Self::ChipZone0 => {
                let f = chip::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.zone_cleared,
                    progress: f.progress(),
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
            Self::WorldCupFirstMatch => {
                let f = worldcup::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.match_won,
                    progress: f.usa_score as u32,
                    active: f.players.each_ref().map(|p| p.active),
                    dead: [false, false],
                    state: serde_json::to_value(f)?,
                })
            }
            Self::DoubleDragonMission1 => {
                let f = dd2::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.mission_cleared,
                    progress: f.substage as u32,
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
            Self::TmntScene1 => {
                let f = tmnt::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.scene_cleared,
                    progress: f.screen as u32,
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
            Self::RiverCityMoose => {
                let f = rcr::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.moose_defeated,
                    progress: f.progress(),
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
            Self::Base1 => {
                let f = base::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.level == 2,
                    progress: f.room as u32,
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
            Self::IceMountain1 => {
                let f = ice::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.completed_task(),
                    progress: f
                        .players
                        .iter()
                        .filter(|p| p.active)
                        .map(|p| p.floor as u32)
                        .max()
                        .unwrap_or(0),
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
            Self::BubbleRound1 => {
                let f = bubble::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.round == 2,
                    progress: 3u32.saturating_sub(f.entities.len() as u32),
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
            Self::SnowFloor1 => {
                let f = snow::observe(m);
                Ok(State {
                    observations: [f.observation(0)?, f.observation(1)?],
                    gameplay: f.gameplay,
                    supported: f.supported,
                    success: f.floor == 1,
                    progress: 4u32.saturating_sub(f.remaining_enemies as u32),
                    active: f.players.each_ref().map(|p| p.active),
                    dead: f.players.each_ref().map(|p| p.dead),
                    state: serde_json::to_value(f)?,
                })
            }
        }
    }
}
#[derive(Serialize)]
pub struct State {
    pub observations: [Vec<f32>; 2],
    pub gameplay: bool,
    pub supported: bool,
    pub success: bool,
    pub progress: u32,
    pub active: [bool; 2],
    pub dead: [bool; 2],
    pub state: serde_json::Value,
}
