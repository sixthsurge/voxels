use std::collections::VecDeque;

use glam::IVec3;
use wgpu::naga::proc::Emitter;

use crate::util::face::FaceIndex;

use self::emitted_light::EmittedLight;

use super::position_types::LocalBlockPosition;

pub mod emitted_light;

pub trait LightStore<LightValue> {
    fn read(&self, pos: LocalBlockPosition) -> LightValue;
    fn write(&mut self, pos: LocalBlockPosition, value: LightValue);
}

#[derive(Clone, Copy, Debug)]
pub struct LightPropagationStep<LightValue> {
    pub position: LocalBlockPosition,
    pub light: LightValue,
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
}

pub type LightUpdatesOutsideChunk = Vec<(FaceIndex, LightUpdate)>;
