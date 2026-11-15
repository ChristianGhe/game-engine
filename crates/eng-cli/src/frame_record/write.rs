//! The record's one layout ([`FrameRecord::to_text`]): keys sorted, each
//! object, probe and unlisted key on a line of its own.

use eng_render::SurfaceClass;
use eng_schema_json::JsonValue;

use super::{
    CameraRecord, FRAME_RECORD_VERSION, FrameEntry, FrameRecord, ObjectRecord, Probe, Unlisted,
    WorldBox, class_name,
};

fn or_null<T>(value: Option<T>, json: impl FnOnce(T) -> JsonValue) -> JsonValue {
    value.map_or(JsonValue::Null, json)
}

fn numbers<T: Copy>(values: &[T], json: impl Fn(T) -> JsonValue) -> JsonValue {
    JsonValue::Array(values.iter().map(|v| json(*v)).collect())
}

fn box_json(b: &WorldBox) -> JsonValue {
    JsonValue::object([
        ("max", numbers(&b.max, JsonValue::F64)),
        ("min", numbers(&b.min, JsonValue::F64)),
    ])
}

impl CameraRecord {
    fn json(&self) -> JsonValue {
        JsonValue::object([
            ("eye", numbers(&self.eye, JsonValue::F64)),
            ("fov_y_deg", JsonValue::F64(self.fov_y_deg)),
            ("near", JsonValue::F32(self.near)),
            ("pitch_deg", JsonValue::F64(self.pitch_deg)),
            ("yaw_deg", JsonValue::F64(self.yaw_deg)),
        ])
    }
}

impl ObjectRecord {
    fn json(&self) -> JsonValue {
        JsonValue::object([
            ("class", JsonValue::str(class_name(self.key.class))),
            ("contrast", or_null(self.contrast, JsonValue::F64)),
            ("id", JsonValue::U64(self.key.id.into())),
            ("luminance", or_null(self.luminance, JsonValue::F64)),
            ("name", or_null(self.name.as_deref(), JsonValue::str)),
            ("pixels", JsonValue::U64(self.pixels)),
            (
                "screen_box",
                or_null(self.screen_box, |b| {
                    numbers(&b, |v| JsonValue::U64(v.into()))
                }),
            ),
            (
                "surround_luminance",
                or_null(self.surround_luminance, JsonValue::F64),
            ),
            ("world_box", box_json(&self.world_box)),
        ])
    }
}

impl Unlisted {
    fn json(&self) -> JsonValue {
        JsonValue::object([
            ("class", JsonValue::str(class_name(self.key.class))),
            ("id", JsonValue::U64(self.key.id.into())),
            ("pixels", JsonValue::U64(self.pixels)),
        ])
    }
}

impl Probe {
    fn json(&self) -> JsonValue {
        JsonValue::object([
            ("class", JsonValue::str(class_name(self.key.class))),
            ("depth", JsonValue::F32(self.depth)),
            ("distance", or_null(self.distance, JsonValue::F64)),
            ("edge", JsonValue::Bool(self.edge)),
            ("id", JsonValue::U64(self.key.id.into())),
            ("slope", JsonValue::F32(self.slope)),
            ("x", JsonValue::U64(self.x.into())),
            ("y", JsonValue::U64(self.y.into())),
        ])
    }
}

/// The classes in name order, as `classes` writes its keys.
fn classes_by_name() -> Vec<SurfaceClass> {
    let mut classes = SurfaceClass::ALL.to_vec();
    classes.sort_by_key(|class| class_name(*class));
    classes
}

/// Appends `items`, one compact JSON value per line at `indent`, as an
/// array that opens on the current line.
fn lines(out: &mut String, indent: &str, items: impl ExactSizeIterator<Item = JsonValue>) {
    if items.len() == 0 {
        out.push_str("[]");
        return;
    }
    out.push('[');
    let count = items.len();
    for (at, item) in items.enumerate() {
        out.push('\n');
        out.push_str(indent);
        out.push_str("  ");
        out.push_str(&item.to_compact());
        if at + 1 < count {
            out.push(',');
        }
    }
    out.push('\n');
    out.push_str(indent);
    out.push(']');
}

impl FrameEntry {
    fn write(&self, out: &mut String) {
        const IN: &str = "      ";
        let key = |out: &mut String, name: &str| {
            out.push_str(IN);
            out.push('"');
            out.push_str(name);
            out.push_str("\": ");
        };
        out.push_str("    {\n");
        key(out, "camera");
        out.push_str(&or_null(self.camera, |c| c.json()).to_compact());
        out.push_str(",\n");
        key(out, "classes");
        let classes = self.classes.map(|bands| {
            JsonValue::object(classes_by_name().into_iter().map(|class| {
                let counts = bands[usize::from(class.code())];
                (class_name(class), numbers(&counts, JsonValue::U64))
            }))
        });
        out.push_str(&or_null(classes, |c| c).to_compact());
        out.push_str(",\n");
        key(out, "histogram");
        out.push_str(&numbers(&self.histogram, JsonValue::U64).to_compact());
        out.push_str(",\n");
        key(out, "invalid_info");
        out.push_str(&JsonValue::U64(self.invalid_info).to_compact());
        out.push_str(",\n");
        key(out, "luminance");
        out.push_str(&JsonValue::F64(self.luminance).to_compact());
        out.push_str(",\n");
        key(out, "objects");
        lines(out, IN, self.objects.iter().map(ObjectRecord::json));
        out.push_str(",\n");
        key(out, "probes");
        lines(out, IN, self.probes.iter().map(Probe::json));
        out.push_str(",\n");
        key(out, "tick");
        out.push_str(&JsonValue::U64(self.tick).to_compact());
        out.push_str(",\n");
        key(out, "unlisted");
        lines(out, IN, self.unlisted.iter().map(Unlisted::json));
        out.push_str("\n    }");
    }
}

impl FrameRecord {
    /// The record as `<out>.frame.json` holds it: keys sorted, each object,
    /// probe and unlisted key on one line, numbers as eng-schema-json
    /// writes them (the shortest form that reads back to the same value),
    /// ending with a newline. The same record gives the same bytes.
    pub fn to_text(&self) -> String {
        let mut out = String::from("{\n  \"frames\": [");
        for (at, frame) in self.frames.iter().enumerate() {
            out.push_str(if at == 0 { "\n" } else { ",\n" });
            frame.write(&mut out);
        }
        out.push_str(if self.frames.is_empty() {
            "],\n"
        } else {
            "\n  ],\n"
        });
        let tail = [
            ("height", JsonValue::U64(self.height.into())),
            ("version", JsonValue::U64(FRAME_RECORD_VERSION)),
            ("view", JsonValue::str(self.view.name())),
            ("width", JsonValue::U64(self.width.into())),
        ];
        for (at, (name, value)) in tail.iter().enumerate() {
            out.push_str("  \"");
            out.push_str(name);
            out.push_str("\": ");
            out.push_str(&value.to_compact());
            out.push_str(if at + 1 < tail.len() { ",\n" } else { "\n" });
        }
        out.push_str("}\n");
        out
    }
}
