#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightLevels {
    pub artificial_light_r: u16,
    pub artificial_light_g: u16,
    pub artificial_light_b: u16,
    pub skylight: u16,
}

impl LightLevels {
    /// Packs the individual light levels as a `PackedLightLevels`
    /// Assumes that the light levels are less than 16
    pub fn packed(&self) -> PackedLightLevels {
        let packed = self.artificial_light_r
            | self.artificial_light_g << 4
            | self.artificial_light_b << 8
            | self.skylight << 12;

        PackedLightLevels(packed)
    }
}

/// 4 light levels packed in 16 bits
/// Bits  0-4  | Artificial light R
/// Bits  4-8  | Artificial light G
/// Bits  8-12 | Artificial light B
/// Bits 12-16 | Skylight
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PackedLightLevels(u16);

impl PackedLightLevels {
    /// All light levels of 0
    pub const ZERO: Self = Self(0);

    /// Returns the underlying `u16` encoding the 4 light levels
    pub fn as_u16(&self) -> u16 {
        self.0
    }

    /// Returns the 4 light levels encoded in this `PackedLightLevels`
    pub fn unpacked(&self) -> LightLevels {
        LightLevels {
            artificial_light_r: (self.0 >> 0) & 15,
            artificial_light_g: (self.0 >> 4) & 15,
            artificial_light_b: (self.0 >> 8) & 15,
            skylight: (self.0 >> 12) & 15,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_packed_light_levels() {
        let light_levels = LightLevels {
            artificial_light_r: 1,
            artificial_light_g: 2,
            artificial_light_b: 3,
            skylight: 4,
        };

        let packed = light_levels.packed();
        let unpacked = packed.unpacked();
        assert_eq!(light_levels, unpacked);
    }
}
