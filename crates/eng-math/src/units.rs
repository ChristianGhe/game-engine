//! Physical units (spec D8): SI and photometric.

use eng_schema_core::Schema;

use crate::Real;

/// Length in meters.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Schema)]
#[schema(name = "eng.math.Meters", version = 1)]
pub struct Meters(pub Real);

/// Illuminance in lux (lumens per square meter).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Schema)]
#[schema(name = "eng.math.Lux", version = 1)]
pub struct Lux(pub Real);

/// Luminance in nits (candela per square meter).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Schema)]
#[schema(name = "eng.math.Nits", version = 1)]
pub struct Nits(pub Real);

/// Camera exposure value at ISO 100.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default, Schema)]
#[schema(name = "eng.math.Ev100", version = 1)]
pub struct Ev100(pub Real);
