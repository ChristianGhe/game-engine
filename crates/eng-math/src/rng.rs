use eng_schema_core::Schema;

/// PCG32 (PCG-XSH-RR 64/32, O'Neill 2014), frozen in rung 1a.
///
/// Gameplay and cosmetic randomness use separate generators (Doom's
/// `P_Random` and `M_Random`); seeds come from the session header.
#[derive(Debug, Clone, PartialEq, Eq, Schema)]
#[schema(name = "eng.math.Pcg32", version = 1)]
pub struct Pcg32 {
    state: u64,
    inc: u64,
}

const MULTIPLIER: u64 = 6_364_136_223_846_793_005;

impl Pcg32 {
    /// Seeds a generator; `stream` selects one of 2^63 independent sequences.
    /// Matches the reference `pcg32_srandom_r(seed, stream)`.
    pub fn new(seed: u64, stream: u64) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (stream << 1) | 1,
        };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(seed);
        rng.next_u32();
        rng
    }

    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULTIPLIER).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// An unbiased value in `0..bound`; `bound` must be non-zero.
    /// Matches the reference `pcg32_boundedrand_r`.
    pub fn next_bounded(&mut self, bound: u32) -> u32 {
        assert!(bound > 0, "Pcg32::next_bounded needs a non-zero bound");
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return r % bound;
            }
        }
    }

    /// A value in `[0, 1)` with 24 random bits.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// A value in `[0, 1)` with 53 random bits.
    pub fn next_f64(&mut self) -> f64 {
        let hi = u64::from(self.next_u32()) << 21;
        let lo = u64::from(self.next_u32() >> 11);
        (hi | lo) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }
}
