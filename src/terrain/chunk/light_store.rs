use super::CHUNK_SIZE_CUBED;
use crate::terrain::{
    lighting::{
        emitted_light::EmittedLight,
        skylight::{self, Skylight},
        LightStore,
    },
    position_types::LocalBlockPosition,
};

#[derive(Clone, Debug)]
pub enum ChunkLightStore {
    AwaitingLightData,
    UniformSkylight(Skylight),
    SkylightOnly(Box<[DoubleSkylight]>),
    EmissionAndSkylight(Box<[EmissionAndSkylight]>),
}

impl ChunkLightStore {
    pub fn new() -> Self {
        Self::AwaitingLightData
    }

    pub fn get_emitted_light(&self, pos: LocalBlockPosition) -> EmittedLight {
        match self {
            Self::AwaitingLightData => EmittedLight::ZERO,
            Self::UniformSkylight(_) | Self::SkylightOnly(_) => EmittedLight::ZERO,
            Self::EmissionAndSkylight(emission_and_skylight) => {
                emission_and_skylight[pos.get_array_index()].get_emission()
            }
        }
    }

    pub fn set_emitted_light(&mut self, pos: LocalBlockPosition, value: EmittedLight) {
        match self {
            Self::EmissionAndSkylight(emission_and_skylight) => {
                let index = pos.get_array_index();
                emission_and_skylight[index] = emission_and_skylight[index].with_emission(value)
            }
            _ => {
                *self = self.promote_to_emission_and_skylight();
                self.set_emitted_light(pos, value);
            }
        }
    }

    pub fn get_skylight(&self, pos: LocalBlockPosition) -> Skylight {
        match self {
            Self::AwaitingLightData => Skylight::ZERO,
            Self::UniformSkylight(skylight) => *skylight,
            Self::SkylightOnly(double_skylight) => {
                let index = pos.get_array_index();
                let skylight = double_skylight[index >> 1];

                if index & 1 == 0 {
                    skylight.get_lower()
                } else {
                    skylight.get_upper()
                }
            }
            Self::EmissionAndSkylight(emission_and_skylight) => {
                emission_and_skylight[pos.get_array_index()].get_skylight()
            }
        }
    }

    pub fn set_skylight(&mut self, pos: LocalBlockPosition, value: Skylight) {
        match self {
            Self::SkylightOnly(double_skylight) => {
                let index = pos.get_array_index();

                if index & 1 == 0 {
                    double_skylight[index >> 1] = double_skylight[index >> 1].with_lower(value)
                } else {
                    double_skylight[index >> 1] = double_skylight[index >> 1].with_upper(value)
                }
            }
            Self::EmissionAndSkylight(emission_and_skylight) => {
                let index = pos.get_array_index();
                emission_and_skylight[index] = emission_and_skylight[index].with_skylight(value)
            }
            Self::UniformSkylight(skylight) if *skylight == value => (),
            _ => {
                *self = self.promote_to_skylight_only();
                self.set_skylight(pos, value);
            }
        }
    }

    pub(super) fn set_full_skylight(&mut self) {
        *self = Self::UniformSkylight(Skylight(Skylight::MAX_VALUE));
    }

    fn promote_to_skylight_only(&mut self) -> Self {
        match self {
            Self::AwaitingLightData => {
                Self::SkylightOnly(vec![DoubleSkylight(0); CHUNK_SIZE_CUBED / 2].into_boxed_slice())
            }
            Self::UniformSkylight(skylight) => Self::SkylightOnly(
                vec![DoubleSkylight::pack(*skylight, *skylight); CHUNK_SIZE_CUBED / 2]
                    .into_boxed_slice(),
            ),
            Self::SkylightOnly(_) => {
                panic!("`promote_to_skylight_only` called with SkylightOnly")
            }
            Self::EmissionAndSkylight(_) => {
                panic!("`promote_to_skylight_only` called with EmissionAndSkylight")
            }
        }
    }

    fn promote_to_emission_and_skylight(&mut self) -> Self {
        match self {
            Self::AwaitingLightData => Self::EmissionAndSkylight(
                vec![EmissionAndSkylight(0); CHUNK_SIZE_CUBED].into_boxed_slice(),
            ),
            Self::UniformSkylight(skylight) => Self::EmissionAndSkylight(
                vec![EmissionAndSkylight::pack(EmittedLight::ZERO, *skylight); CHUNK_SIZE_CUBED]
                    .into_boxed_slice(),
            ),
            Self::SkylightOnly(double_skylight) => Self::EmissionAndSkylight(
                double_skylight
                    .iter()
                    .map(|double_skylight| {
                        [
                            EmissionAndSkylight::pack(
                                EmittedLight::ZERO,
                                double_skylight.get_lower(),
                            ),
                            EmissionAndSkylight::pack(
                                EmittedLight::ZERO,
                                double_skylight.get_upper(),
                            ),
                        ]
                    })
                    .flatten()
                    .collect(),
            ),
            Self::EmissionAndSkylight(_) => {
                panic!("`promote_to_emission_and_skylight` called with EmissionAndSkylight")
            }
        }
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

/// Two skylight values packed in 8 bits
#[derive(Clone, Copy, Debug)]
pub struct DoubleSkylight(u8);

impl DoubleSkylight {
    pub fn pack(lower: Skylight, upper: Skylight) -> Self {
        Self(lower.0 | (upper.0 << 4))
    }

    pub fn get_lower(self) -> Skylight {
        Skylight(self.0 & 0b1111)
    }

    pub fn get_upper(self) -> Skylight {
        Skylight((self.0 >> 4) & 0b1111)
    }

    pub fn with_lower(self, lower: Skylight) -> Self {
        Self((self.0 & 0b11110000) | lower.0)
    }

    pub fn with_upper(self, lower: Skylight) -> Self {
        Self((self.0 & 0b00001111) | (lower.0 << 4))
    }
}

/// Skylight and emission values packed in 16 bits
#[derive(Clone, Copy, Debug)]
pub struct EmissionAndSkylight(u16);

impl EmissionAndSkylight {
    pub fn pack(emission: EmittedLight, skylight: Skylight) -> Self {
        Self(emission.0 | ((skylight.0 as u16) << 12))
    }

    pub fn get_emission(self) -> EmittedLight {
        EmittedLight(self.0 & 0b0000_1111_1111_1111)
    }

    pub fn get_skylight(self) -> Skylight {
        Skylight(((self.0 >> 12) & 0b1111) as u8)
    }

    pub fn with_emission(mut self, emission: EmittedLight) -> Self {
        self.0 &= 0b1111_0000_0000_0000;
        self.0 |= emission.0;
        self
    }

    pub fn with_skylight(mut self, skylight: Skylight) -> Self {
        self.0 &= 0b0000_1111_1111_1111;
        self.0 |= (skylight.0 as u16) << 12;
        self
    }
}
