use std::collections::VecDeque;

use self::{emitted_light::EmittedLight, skylight::Skylight};
use super::position_types::LocalBlockPosition;
use crate::util::face::FaceIndex;

pub mod emitted_light;
pub mod skylight;

pub trait LightStore<LightValue> {
    fn read(&self, pos: LocalBlockPosition) -> LightValue;
    fn write(&mut self, pos: LocalBlockPosition, value: LightValue);
}

#[derive(Clone, Copy, Debug)]
pub struct LightPropagationStep<LightValue> {
    pub position: LocalBlockPosition,
    pub light: LightValue,
    pub is_repair_step: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct ShadowPropagationStep {
    pub position: LocalBlockPosition,
    pub depth: u32,
}

pub type LightPropagationQueue<LightValue> = VecDeque<LightPropagationStep<LightValue>>;
pub type ShadowPropagationQueue = VecDeque<ShadowPropagationStep>;

pub enum LightUpdate {
    EmittedLight(LightPropagationStep<EmittedLight>),
    EmittedLightShadow(ShadowPropagationStep),
    Skylight(LightPropagationStep<Skylight>),
}

pub type LightUpdatesOutsideChunk = Vec<(FaceIndex, LightUpdate)>;
