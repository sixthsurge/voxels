use super::CHUNK_SIZE_CUBED;
use crate::terrain::{
    lighting::{emitted_light::EmittedLight, skylight::Skylight, LightStore},
    position_types::LocalBlockPosition,
};

#[derive(Clone, Debug)]
pub struct ChunkLightStore {
    emitted_light: Box<[EmittedLight]>,
    skylight: Box<[Skylight]>,
}

impl ChunkLightStore {
    pub fn new() -> Self {
        Self {
            emitted_light: vec![EmittedLight::ZERO; CHUNK_SIZE_CUBED].into_boxed_slice(),
            skylight: vec![Skylight::ZERO; CHUNK_SIZE_CUBED].into_boxed_slice(),
        }
    }

    pub fn get_emitted_light(&self, pos: LocalBlockPosition) -> EmittedLight {
        self.emitted_light[pos.get_array_index()]
    }

    pub fn set_emitted_light(&mut self, pos: LocalBlockPosition, value: EmittedLight) {
        self.emitted_light[pos.get_array_index()] = value;
    }

    pub fn get_skylight(&self, pos: LocalBlockPosition) -> Skylight {
        self.skylight[pos.get_array_index()]
    }

    pub fn set_skylight(&mut self, pos: LocalBlockPosition, value: Skylight) {
        self.skylight[pos.get_array_index()] = value;
    }
}

impl LightStore<EmittedLight> for ChunkLightStore {
    fn read(&self, pos: LocalBlockPosition) -> EmittedLight {
        self.get_emitted_light(pos)
    }

    fn write(&mut self, pos: LocalBlockPosition, value: EmittedLight) {
        self.set_emitted_light(pos, value)
    }
}

impl LightStore<Skylight> for ChunkLightStore {
    fn read(&self, pos: LocalBlockPosition) -> Skylight {
        self.get_skylight(pos)
    }

    fn write(&mut self, pos: LocalBlockPosition, value: Skylight) {
        self.set_skylight(pos, value)
    }
}
