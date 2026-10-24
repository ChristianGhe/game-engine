use eng_math::*;
use eng_schema_core::{from_bytes, to_bytes};
use proptest::prelude::*;

#[test]
fn fnv1a64_reference_vectors() {
    // From the FNV reference test suite (isthe.com/chongo/tech/comp/fnv).
    assert_eq!(Fnv1a64::hash(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(Fnv1a64::hash(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(Fnv1a64::hash(b"foobar"), 0x8594_4171_f739_67e8);
}

#[test]
fn pcg32_reference_vectors() {
    // `pcg32-demo` from pcg-c-basic, seeded with (42, 54).
    let mut rng = Pcg32::new(42, 54);
    let got: Vec<u32> = (0..6).map(|_| rng.next_u32()).collect();
    assert_eq!(
        got,
        [
            0xa15c02b7, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e
        ]
    );
}

#[test]
fn pcg32_roundtrips_through_schema() {
    let mut rng = Pcg32::new(7, 3);
    rng.next_u32();
    let mut back: Pcg32 = from_bytes(&to_bytes(&rng)).unwrap();
    assert_eq!(back.next_u32(), rng.next_u32());
}

#[test]
fn signed_zero_rules() {
    assert_eq!(min(0.0, -0.0).to_bits(), (-0.0f32).to_bits());
    assert_eq!(min(-0.0, 0.0).to_bits(), 0.0f32.to_bits());
    assert_eq!(max(0.0, -0.0).to_bits(), (-0.0f32).to_bits());
    assert!(min(f32::NAN, 1.0) == 1.0);
    assert!(clamp(f32::NAN, 0.0, 1.0).is_nan());
}

#[test]
fn binary_angles() {
    assert_eq!(BAngle::from_u16(0x4000), BAngle::QUARTER);
    assert_eq!(BAngle::QUARTER.to_u16(), 0x4000);
    assert_eq!(BAngle(u32::MAX).wrapping_add(BAngle(2)), BAngle(1));
    assert_eq!(BAngle(10).delta(BAngle(u32::MAX)), 11);
    let d = BAngle::QUARTER.direction();
    assert!(d.x.abs() < 1e-7 && (d.y - 1.0).abs() < 1e-7);
    let d = BAngle::HALF.direction();
    assert!((d.x + 1.0).abs() < 1e-7 && d.y.abs() < 1e-7);
}

#[test]
fn world_pos_is_camera_relative_in_f64() {
    let far = WorldPos::new(1.0e7 + 0.25, -3.0, 0.0);
    let camera = WorldPos::new(1.0e7, 0.0, 0.0);
    assert_eq!(far.relative_to(camera), Vec3::new(0.25, -3.0, 0.0));
}

proptest! {
    #[test]
    fn min_max_agree_with_ordering(a in -1e6f32..1e6, b in -1e6f32..1e6) {
        prop_assert!(min(a, b) <= max(a, b));
        prop_assert!(min(a, b) == a || min(a, b) == b);
    }

    #[test]
    fn bounded_stays_in_range(seed: u64, stream: u64, bound in 1u32..) {
        let mut rng = Pcg32::new(seed, stream);
        prop_assert!(rng.next_bounded(bound) < bound);
        let f = rng.next_f32();
        prop_assert!((0.0..1.0).contains(&f));
        let g = rng.next_f64();
        prop_assert!((0.0..1.0).contains(&g));
    }

    #[test]
    fn normalize_is_unit_or_zero(x in -1e3f32..1e3, y in -1e3f32..1e3, z in -1e3f32..1e3) {
        let n = Vec3::new(x, y, z).normalize_or_zero();
        let len = n.length();
        prop_assert!(len == 0.0 || (len - 1.0).abs() < 1e-5);
    }
}
