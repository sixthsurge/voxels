/// Emitted light values for one block packed in 16 bits
/// Bits 0 to 4   | Red component
/// Bits 4 to 8   | Green component
/// Bits 8 to 12  | Blue component
/// Bits 12 to 16 | Unused (can store skylight!)
#[derive(Clone, Copy, Debug)]
pub struct EmittedLight(u16);

impl EmittedLight {
    const COMPONENT_MASK: u16 = 0x0f0f;
    const BORROW_GUARD: u16 = 0x2020;
    const CARRY_MASK: u16 = 0x1010;

    /// Wrap a u16 storing the 3 light values in an `EmittedLight`
    pub fn from_u16(value: u16) -> Self {
        Self(value)
    }

    /// Created a packed `EmittedLight` value from the 3 light values
    /// Values must be in 0..16
    pub fn from_rgb(r: u16, g: u16, b: u16) -> Self {
        debug_assert!((0..16).contains(&r));
        debug_assert!((0..16).contains(&g));
        debug_assert!((0..16).contains(&b));

        Self(r | g << 4 | b << 8)
    }

    /// Returns the underlying u16 storing the 3 light values
    pub fn as_u16(&self) -> u16 {
        self.0
    }

    /// Returns the individual RGB light values represented by this packed `EmittedLight` value
    /// Values are in 0..16
    pub fn as_rgb(&self) -> (u16, u16, u16) {
        ((self.0 >> 0) & 15, (self.0 >> 4) & 15, (self.0 >> 8) & 15)
    }

    /// Pair-wise < operation
    /// Compare two sets of light values and determine which components are < the other
    pub fn less(a: Self, b: Self) -> u16 {
        // https://0fps.net/2018/02/21/voxel-lighting/ (I did not invent this)
        Self::half_less(a.0, b.0) | (Self::half_less(a.0 >> 4, b.0 >> 4) << 4)
    }

    /// Pair-wise `max` operation
    pub fn max(a: Self, b: Self) -> Self {
        // https://0fps.net/2018/02/21/voxel-lighting/ (I did not invent this)
        let result = a.0 ^ ((a.0 ^ b.0) & Self::less(a, b));
        Self(result)
    }

    /// Subtract one from each component, saturating on underflow
    pub fn decrement_and_saturate(&self) -> EmittedLight {
        let result = Self::decrement_and_saturate_half(self.0)
            | (Self::decrement_and_saturate_half(self.0 >> 4) << 4);
        Self(result)
    }

    fn half_less(a: u16, b: u16) -> u16 {
        // https://0fps.net/2018/02/21/voxel-lighting/ (I did not invent this)
        let d = (((a & Self::COMPONENT_MASK) | Self::BORROW_GUARD) - (b & Self::COMPONENT_MASK))
            & Self::CARRY_MASK;
        (d >> 1) | (d >> 2) | (d >> 3) | (d >> 4)
    }

    fn decrement_and_saturate_half(x: u16) -> u16 {
        // https://0fps.net/2018/02/21/voxel-lighting/ (I did not invent this)

        // compute component-wise decrement
        let d = ((x & Self::COMPONENT_MASK) | Self::BORROW_GUARD) - 0x0101;

        // check for underflow
        let b = d & Self::CARRY_MASK;

        // saturate underflowed values
        (d + (b >> 4)) & Self::COMPONENT_MASK
    }
}

/// Skylight value for one block
#[derive(Clone, Copy, Debug)]
pub struct Skylight(u8);

#[cfg(test)]
mod tests {
    use super::EmittedLight;

    #[test]
    fn emitted_light_from_and_as_rgb() {
        let r = 1;
        let g = 2;
        let b = 3;
        let emitted_light = EmittedLight::from_rgb(r, g, b);
        assert_eq!((r, g, b), emitted_light.as_rgb())
    }

    #[test]
    fn emitted_light_less() {
        {
            let a = EmittedLight::from_rgb(1, 0, 1);
            let b = EmittedLight::from_rgb(0, 1, 0);
            assert_eq!(0b0000_1111_0000, EmittedLight::less(a, b));
        }

        {
            let a = EmittedLight::from_rgb(0, 0, 0);
            let b = EmittedLight::from_rgb(15, 15, 15);
            assert_eq!(0b1111_1111_1111, EmittedLight::less(a, b));
        }

        {
            let a = EmittedLight::from_rgb(0, 1, 2);
            let b = EmittedLight::from_rgb(0, 1, 2);
            assert_eq!(0b0000_0000_0000, EmittedLight::less(a, b));
        }
    }

    #[test]
    fn emitted_light_max() {
        let a = EmittedLight::from_rgb(15, 10, 5);
        let b = EmittedLight::from_rgb(5, 10, 15);
        assert_eq!((15, 10, 15), EmittedLight::max(a, b).as_rgb())
    }

    #[test]
    fn emitted_light_decrement_and_saturate() {
        let x = EmittedLight::from_rgb(0, 1, 2);
        assert_eq!((0, 0, 1), x.decrement_and_saturate().as_rgb())
    }
}
