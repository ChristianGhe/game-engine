//! Shader checks (spec 6.7; rung 1b plan, decision 11; acceptance feature
//! `shaders`): every WGSL file in the repo validates with naga and translates
//! to SPIR-V, MSL and HLSL. A shader that only one backend accepts then fails
//! on every runner, not only on the machines with that backend.
//!
//! The options are what eng-gpu's devices guarantee: Tier 0, which is
//! WebGPU compliance (spec 6.7), on the lowest version of each backend.
//! - Validation with the capabilities wgpu 30 gives naga for no features and
//!   compliant downlevel flags (wgpu-naga-bridge's
//!   `features_to_naga_capabilities`): naga's defaults plus half floats
//!   packed in f32 (`quantizeToF16`, `pack2x16float`, `unpack2x16float`).
//! - Pipeline-overridable constants take their defaults, as in a wgpu
//!   pipeline that sets none.
//! - SPIR-V 1.0, naga's default and the lowest a Vulkan device takes.
//! - MSL 2.1: wgpu counts Metal below it as short of WebGPU compliance
//!   (`DownlevelFlags::MSL2_1`).
//! - HLSL shader model 5.1, the most FXC compiles. eng-gpu gives wgpu's DX12
//!   backend FXC, which ships with Windows (decision 10).
//!
//! The rejected shaders below pin naga 30's behaviour, so a wgpu upgrade
//! may need them changed too.

use std::fs;
use std::path::{Path, PathBuf};

use naga::Module;
use naga::back::{PipelineConstants, hlsl, msl, pipeline_constants, spv};
use naga::valid::{Capabilities, ModuleInfo, ValidationFlags, Validator};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the crate sits two levels below the repo root")
}

/// Every `*.wgsl` file below `root`, in path order. At the root only, it
/// skips `target` and hidden directories (`.git`, and `.claude`, which may
/// hold other checkouts); deeper down it looks everywhere.
fn wgsl_files(root: &Path, out: &mut Vec<PathBuf>) {
    walk(root, root, out);
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            if dir != root || (name != "target" && !name.starts_with('.')) {
                walk(root, &path, out);
            }
        } else if path.extension().is_some_and(|ext| ext == "wgsl") {
            out.push(path);
        }
    }
}

/// Tier 0's capabilities (see the top of this file).
fn tier_0() -> Capabilities {
    Capabilities::default() | Capabilities::SHADER_FLOAT16_IN_FLOAT32
}

fn validate(module: &Module, capabilities: Capabilities) -> Result<ModuleInfo, String> {
    Validator::new(ValidationFlags::all(), capabilities)
        .validate(module)
        .map_err(|e| format!("validation: {e}"))
}

fn spirv(module: &Module, info: &ModuleInfo) -> Result<(), String> {
    spv::write_vec(module, info, &spv::Options::default(), None)
        .map(drop)
        .map_err(|e| format!("SPIR-V: {e}"))
}

fn msl(module: &Module, info: &ModuleInfo) -> Result<(), String> {
    let options = msl::Options {
        lang_version: (2, 1),
        ..msl::Options::default()
    };
    msl::write_string(module, info, &options, &msl::PipelineOptions::default())
        .map(drop)
        .map_err(|e| format!("MSL: {e}"))
}

fn hlsl(module: &Module, info: &ModuleInfo) -> Result<(), String> {
    let options = hlsl::Options {
        shader_model: hlsl::ShaderModel::V5_1,
        ..hlsl::Options::default()
    };
    let mut source = String::new();
    hlsl::Writer::new(&mut source, &options, &hlsl::PipelineOptions::default())
        .write(module, info, None)
        .map(drop)
        .map_err(|e| format!("HLSL: {e}"))
}

/// Parses and validates `source`, gives its overridable constants their
/// defaults, then translates it for each backend. The error starts with the
/// step that failed: `WGSL`, `validation`, `overrides`, `SPIR-V`, `MSL` or
/// `HLSL`.
fn check(source: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(source)
        .map_err(|e| format!("WGSL: {}", e.emit_to_string(source)))?;
    let info = Validator::new(ValidationFlags::all(), tier_0())
        .validate(&module)
        .map_err(|e| format!("validation: {}", e.emit_to_string(source)))?;
    let (module, info) =
        pipeline_constants::process_overrides(&module, &info, None, &PipelineConstants::default())
            .map_err(|e| format!("overrides: {e}"))?;
    spirv(&module, &info)?;
    msl(&module, &info)?;
    hlsl(&module, &info)
}

/// The step `check` stopped at.
fn failed_step(source: &str) -> String {
    let error = check(source).expect_err("the shader should fail");
    error.split(':').next().unwrap_or_default().to_string()
}

#[test]
fn every_wgsl_file_validates_and_translates() {
    let root = repo_root();
    let mut files = Vec::new();
    wgsl_files(&root, &mut files);
    // The scan reaches the crates, where eng-gpu's own vertex stage lives.
    assert!(
        files.iter().any(|f| f.starts_with(root.join("crates"))),
        "the scan of {} found no WGSL file under crates/: {files:?}",
        root.display()
    );
    let failures: Vec<String> = files
        .iter()
        .filter_map(|file| {
            let shown = file.strip_prefix(&root).unwrap_or(file).display();
            match fs::read_to_string(file) {
                Ok(source) => check(&source).err().map(|e| format!("{shown}: {e}")),
                Err(e) => Some(format!("{shown}: {e}")),
            }
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} WGSL files fail:\n{}",
        failures.len(),
        files.len(),
        failures.join("\n")
    );
    eprintln!(
        "shaders: {} WGSL files validate and translate to SPIR-V, MSL and HLSL",
        files.len()
    );
}

#[test]
fn the_check_stops_at_the_step_that_rejects_a_shader() {
    assert_eq!(failed_step("@fragment fn main( {"), "WGSL");
    // Valid WGSL, but half floats are an optional feature, not Tier 0.
    assert_eq!(
        failed_step(
            "enable f16;\n\
             @fragment fn main() -> @location(0) vec4<f32> {\n\
                 let x: f16 = 1.0h;\n\
                 return vec4<f32>(f32(x));\n\
             }"
        ),
        "validation"
    );
    // An overridable constant without a default needs a value from the
    // pipeline, and a wgpu pipeline that sets none fails the same way.
    assert_eq!(
        failed_step(
            "override scale: f32;\n\
             @vertex fn main() -> @builtin(position) vec4<f32> {\n\
                 return vec4<f32>(scale, 0.0, 0.0, 1.0);\n\
             }"
        ),
        "overrides"
    );
    // Every step accepts these, which Tier 0 allows: instancing, an
    // override with a default, `@invariant`, and half floats packed in f32.
    assert_eq!(
        check(
            "override scale: f32 = 2.0;\n\
             @vertex fn main(@builtin(instance_index) i: u32) -> @builtin(position) @invariant vec4<f32> {\n\
                 let x = quantizeToF16(f32(i) * scale);\n\
                 return vec4<f32>(x, unpack2x16float(pack2x16float(vec2<f32>(x))).x, 0.0, 1.0);\n\
             }"
        ),
        Ok(())
    );
}

/// A scratch tree outside the repo, removed when dropped.
struct ScratchTree(PathBuf);

impl ScratchTree {
    fn new(files: &[&str]) -> Self {
        let root = std::env::temp_dir().join(format!("eng-gpu-shader-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for file in files {
            let path = root.join(file);
            fs::create_dir_all(path.parent().expect("a parent")).expect("a scratch directory");
            fs::write(&path, "").expect("a scratch file");
        }
        Self(root)
    }
}

impl Drop for ScratchTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn the_scan_skips_only_the_build_and_hidden_directories_at_the_root() {
    let tree = ScratchTree::new(&[
        "top.wgsl",
        "notes.txt",
        "target/debug/built.wgsl",
        ".git/kept-by-git.wgsl",
        ".claude/worktrees/other/crates/a/src/other.wgsl",
        "crates/a/src/pass.wgsl",
        "crates/a/src/target/included.wgsl",
        "crates/a/.shaders/hidden.wgsl",
    ]);
    let mut found = Vec::new();
    wgsl_files(&tree.0, &mut found);
    let found: Vec<String> = found
        .iter()
        .map(|f| {
            f.strip_prefix(&tree.0)
                .expect("inside")
                .display()
                .to_string()
        })
        .collect();
    assert_eq!(
        found,
        [
            "crates/a/.shaders/hidden.wgsl",
            "crates/a/src/pass.wgsl",
            "crates/a/src/target/included.wgsl",
            "top.wgsl",
        ]
    );
}

#[test]
fn each_backend_reports_what_it_cannot_translate() {
    // `check` resolves overridable constants first, as wgpu does for each
    // pipeline; each backend refuses one left unresolved.
    let source = "override scale: f32 = 2.0;\n\
                  @vertex fn main() -> @builtin(position) vec4<f32> {\n\
                      return vec4<f32>(scale, 0.0, 0.0, 1.0);\n\
                  }";
    let module = naga::front::wgsl::parse_str(source).expect("valid WGSL");
    let info = validate(&module, tier_0()).expect("valid at Tier 0");
    for (backend, result) in [
        ("SPIR-V", spirv(&module, &info)),
        ("MSL", msl(&module, &info)),
        ("HLSL", hlsl(&module, &info)),
    ] {
        let error = result.expect_err(backend);
        assert!(error.starts_with(&format!("{backend}: ")), "{error}");
    }
    // The floors, with features beyond Tier 0 so that validation passes:
    // views need HLSL shader model 6.1, and primitive indices MSL 2.3.
    let beyond = |source: &str| {
        let module = naga::front::wgsl::parse_str(source).expect("valid WGSL");
        let info = validate(&module, Capabilities::all()).expect("valid with every capability");
        (module, info)
    };
    let (views, info) = beyond(
        "@fragment fn main(@builtin(view_index) view: u32) -> @location(0) vec4<f32> {\n\
             return vec4<f32>(f32(view));\n\
         }",
    );
    assert!(spirv(&views, &info).is_ok());
    let error = hlsl(&views, &info).expect_err("shader model 5.1 has no views");
    assert!(error.starts_with("HLSL: "), "{error}");
    let (primitives, info) = beyond(
        "enable primitive_index;\n\
         @fragment fn main(@builtin(primitive_index) p: u32) -> @location(0) vec4<f32> {\n\
             return vec4<f32>(f32(p));\n\
         }",
    );
    assert!(spirv(&primitives, &info).is_ok());
    let error = msl(&primitives, &info).expect_err("MSL 2.1 has no primitive index");
    assert!(error.starts_with("MSL: "), "{error}");
}
