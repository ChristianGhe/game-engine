//! Known-answer tests (spec 6.4): about 1M seeded inputs per function, with
//! the output bits hashed. The expected hashes in `known_answers.txt` are a
//! protected oracle; they must match on every target CI runs.
//!
//! Each function draws from its own PCG32 stream, keyed by its name, so adding
//! a function never changes another's inputs. NaN outputs are canonicalized
//! before hashing because NaN payloads legitimately differ between CPUs, and
//! simulation state is never NaN anyway.

use std::collections::BTreeMap;

use eng_math::*;

const INPUTS: u32 = 1_000_000;
const SEED: u64 = 0x0e46_6d61_7468_4b41; // "eng-math KA"
const EXPECTED: &str = include_str!("known_answers.txt");

struct Kat {
    rng: Pcg32,
    hash: Fnv1a64,
}

impl Kat {
    fn new(name: &str) -> Self {
        Self {
            rng: Pcg32::new(SEED, Fnv1a64::hash(name.as_bytes())),
            hash: Fnv1a64::new(),
        }
    }

    /// Half raw bit patterns (every class of float), half uniform in `[lo, hi)`.
    fn f32_in(&mut self, lo: f32, hi: f32) -> f32 {
        if self.rng.next_u32() & 1 == 0 {
            f32::from_bits(self.rng.next_u32())
        } else {
            lo + (hi - lo) * self.rng.next_f32()
        }
    }

    fn f64_in(&mut self, lo: f64, hi: f64) -> f64 {
        if self.rng.next_u32() & 1 == 0 {
            let hi_bits = u64::from(self.rng.next_u32()) << 32;
            f64::from_bits(hi_bits | u64::from(self.rng.next_u32()))
        } else {
            lo + (hi - lo) * self.rng.next_f64()
        }
    }

    fn out_f32(&mut self, x: f32) {
        let bits = if x.is_nan() { 0x7fc0_0000 } else { x.to_bits() };
        self.hash.write(&bits.to_le_bytes());
    }

    fn out_f64(&mut self, x: f64) {
        let bits = if x.is_nan() {
            0x7ff8_0000_0000_0000
        } else {
            x.to_bits()
        };
        self.hash.write(&bits.to_le_bytes());
    }
}

type Case = (&'static str, fn(&mut Kat));

fn unary_f32(k: &mut Kat, lo: f32, hi: f32, f: fn(f32) -> f32) {
    for _ in 0..INPUTS {
        let x = k.f32_in(lo, hi);
        k.out_f32(f(x));
    }
}

fn binary_f32(k: &mut Kat, a: (f32, f32), b: (f32, f32), f: fn(f32, f32) -> f32) {
    for _ in 0..INPUTS {
        let x = k.f32_in(a.0, a.1);
        let y = k.f32_in(b.0, b.1);
        k.out_f32(f(x, y));
    }
}

fn unary_f64(k: &mut Kat, lo: f64, hi: f64, f: fn(f64) -> f64) {
    for _ in 0..INPUTS {
        let x = k.f64_in(lo, hi);
        k.out_f64(f(x));
    }
}

fn binary_f64(k: &mut Kat, a: (f64, f64), b: (f64, f64), f: fn(f64, f64) -> f64) {
    for _ in 0..INPUTS {
        let x = k.f64_in(a.0, a.1);
        let y = k.f64_in(b.0, b.1);
        k.out_f64(f(x, y));
    }
}

const CASES: &[Case] = &[
    ("sqrt", |k| unary_f32(k, 0.0, 1e6, sqrt)),
    ("cbrt", |k| unary_f32(k, -1e6, 1e6, cbrt)),
    ("sin", |k| unary_f32(k, -100.0, 100.0, sin)),
    ("cos", |k| unary_f32(k, -100.0, 100.0, cos)),
    ("tan", |k| unary_f32(k, -100.0, 100.0, tan)),
    ("asin", |k| unary_f32(k, -1.0, 1.0, asin)),
    ("acos", |k| unary_f32(k, -1.0, 1.0, acos)),
    ("atan", |k| unary_f32(k, -1e3, 1e3, atan)),
    ("exp", |k| unary_f32(k, -90.0, 90.0, exp)),
    ("exp2", |k| unary_f32(k, -130.0, 130.0, exp2)),
    ("ln", |k| unary_f32(k, 0.0, 1e6, ln)),
    ("log2", |k| unary_f32(k, 0.0, 1e6, log2)),
    ("log10", |k| unary_f32(k, 0.0, 1e6, log10)),
    ("floor", |k| unary_f32(k, -1e4, 1e4, floor)),
    ("ceil", |k| unary_f32(k, -1e4, 1e4, ceil)),
    ("round", |k| unary_f32(k, -1e4, 1e4, round)),
    ("trunc", |k| unary_f32(k, -1e4, 1e4, trunc)),
    ("atan2", |k| {
        binary_f32(k, (-100.0, 100.0), (-100.0, 100.0), atan2)
    }),
    ("pow", |k| binary_f32(k, (0.0, 100.0), (-10.0, 10.0), pow)),
    ("hypot", |k| binary_f32(k, (-1e3, 1e3), (-1e3, 1e3), hypot)),
    ("min", |k| binary_f32(k, (-1.0, 1.0), (-1.0, 1.0), min)),
    ("max", |k| binary_f32(k, (-1.0, 1.0), (-1.0, 1.0), max)),
    ("clamp", |k| {
        for _ in 0..INPUTS {
            let x = k.f32_in(-2.0, 2.0);
            let lo = k.f32_in(-1.0, 0.0);
            k.out_f32(clamp(x, lo, 1.0));
        }
    }),
    ("sin_cos", |k| {
        for _ in 0..INPUTS {
            let (s, c) = sin_cos(k.f32_in(-100.0, 100.0));
            k.out_f32(s);
            k.out_f32(c);
        }
    }),
    ("sqrt_f64", |k| unary_f64(k, 0.0, 1e12, sqrt_f64)),
    ("sin_f64", |k| unary_f64(k, -1e3, 1e3, sin_f64)),
    ("cos_f64", |k| unary_f64(k, -1e3, 1e3, cos_f64)),
    ("floor_f64", |k| unary_f64(k, -1e9, 1e9, floor_f64)),
    ("ceil_f64", |k| unary_f64(k, -1e9, 1e9, ceil_f64)),
    ("round_f64", |k| unary_f64(k, -1e9, 1e9, round_f64)),
    ("trunc_f64", |k| unary_f64(k, -1e9, 1e9, trunc_f64)),
    ("atan2_f64", |k| {
        binary_f64(k, (-1e3, 1e3), (-1e3, 1e3), atan2_f64)
    }),
    ("min_f64", |k| {
        binary_f64(k, (-1.0, 1.0), (-1.0, 1.0), min_f64)
    }),
    ("max_f64", |k| {
        binary_f64(k, (-1.0, 1.0), (-1.0, 1.0), max_f64)
    }),
    ("clamp_f64", |k| {
        for _ in 0..INPUTS {
            let x = k.f64_in(-2.0, 2.0);
            let lo = k.f64_in(-1.0, 0.0);
            k.out_f64(clamp_f64(x, lo, 1.0));
        }
    }),
    ("sin_cos_f64", |k| {
        for _ in 0..INPUTS {
            let (s, c) = sin_cos_f64(k.f64_in(-1e3, 1e3));
            k.out_f64(s);
            k.out_f64(c);
        }
    }),
    ("bangle_sin_cos", |k| {
        for _ in 0..INPUTS {
            let (s, c) = BAngle(k.rng.next_u32()).sin_cos();
            k.out_f32(s);
            k.out_f32(c);
        }
    }),
    ("vec3_normalize", |k| {
        for _ in 0..INPUTS {
            let v = Vec3::new(
                k.f32_in(-1e3, 1e3),
                k.f32_in(-1e3, 1e3),
                k.f32_in(-1e3, 1e3),
            );
            let n = v.normalize_or_zero();
            k.out_f32(n.x);
            k.out_f32(n.y);
            k.out_f32(n.z);
        }
    }),
    ("pcg32_stream", |k| {
        let mut rng = Pcg32::new(42, 54);
        for _ in 0..INPUTS {
            k.hash.write(&rng.next_u32().to_le_bytes());
        }
        for bound in 1..=1000 {
            k.hash.write(&rng.next_bounded(bound).to_le_bytes());
        }
    }),
];

fn expected() -> BTreeMap<&'static str, (u32, u64)> {
    EXPECTED
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(parts.len(), 3, "bad known_answers.txt line: {line}");
            let inputs = parts[1].parse().expect("input count");
            let hash = u64::from_str_radix(parts[2].trim_start_matches("0x"), 16).expect("hash");
            (parts[0], (inputs, hash))
        })
        .collect()
}

#[test]
fn known_answers_match() {
    let expected = expected();
    let mut computed = Vec::new();
    let mut failures = Vec::new();
    for (name, run) in CASES {
        let mut kat = Kat::new(name);
        run(&mut kat);
        let hash = kat.hash.finish();
        computed.push(format!("{name} {INPUTS} {hash:#018x}"));
        match expected.get(name) {
            Some(&(inputs, want)) if inputs == INPUTS && want == hash => {}
            Some(&(inputs, want)) => failures.push(format!(
                "[E-KAT] {name}: expected {inputs} inputs {want:#018x}, computed {INPUTS} inputs {hash:#018x}"
            )),
            None => failures.push(format!("[E-KAT] {name}: missing from known_answers.txt")),
        }
    }
    for name in expected.keys() {
        if !CASES.iter().any(|(n, _)| n == name) {
            failures.push(format!("[E-KAT] {name}: listed but no case computes it"));
        }
    }
    assert!(
        failures.is_empty(),
        "{}\n\nComputed on this target ({} {}):\n{}\n\nknown_answers.txt is protected: only the owner may change it (AGENTS.md).",
        failures.join("\n"),
        std::env::consts::ARCH,
        std::env::consts::OS,
        computed.join("\n"),
    );
}
