//! `cargo xtask arch-check` (spec 6.2): layering, core-set purity over the
//! transitive closure, floor wrappers, crate-root attributes, lint escapes,
//! workspace lints and release overflow checks.
//!
//! The rules run over a [`Model`] so they can be tested on synthetic
//! workspaces; [`load`] builds the model from `cargo metadata`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::util;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchFile {
    #[serde(rename = "version")]
    _version: u32,
    layers: Vec<String>,
    core_third_party: BTreeMap<String, Pin>,
    /// Crates allowed under `[patch]` (spec 7.7: forks need a tracking note).
    #[serde(default)]
    allowed_patches: Vec<String>,
    floor: BTreeMap<String, String>,
    crates: BTreeMap<String, CrateRule>,
}

/// The exact version and enabled features of a core third-party crate.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pin {
    version: String,
    features: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrateRule {
    layer: String,
    #[serde(default)]
    core: bool,
    #[serde(default)]
    proc_macro: bool,
    #[serde(default)]
    same_layer: Vec<String>,
}

/// One package in the dependency graph.
#[derive(Debug, Default, Clone)]
pub struct Package {
    pub workspace: bool,
    pub version: String,
    /// `None` for path dependencies; registry packages name their registry.
    pub source: Option<String>,
    /// Features enabled in the resolved graph.
    pub features: BTreeSet<String>,
    pub has_build_script: bool,
    /// Normal (non-dev, non-build) dependencies, by package name.
    pub deps: BTreeSet<String>,
    pub build_deps: BTreeSet<String>,
    /// Workspace crates only: `src/lib.rs` text, if a library.
    pub lib_rs: Option<String>,
    /// Workspace crates only: the library target's root file, if it is not
    /// `src/lib.rs` (a `[lib] path = ...` override).
    pub odd_lib_path: Option<String>,
    /// Workspace crates only: every `.rs` file under `src/`.
    pub sources: Vec<(String, String)>,
    /// Workspace crates only: `[lints] workspace = true`.
    pub inherits_lints: bool,
}

pub struct Model {
    pub rules: ArchFile,
    pub packages: BTreeMap<String, Package>,
    /// Packages with `[profile.release.package.<name>] overflow-checks = true`.
    pub overflow_checked: BTreeSet<String>,
    /// Crates under `[patch]` in the root Cargo.toml (`[replace]` counts too).
    pub patches: Vec<String>,
    /// Crate-level clippy configurations, which would replace the root ban list.
    pub clippy_configs: Vec<String>,
}

fn layer_index(rules: &ArchFile, layer: &str) -> Option<usize> {
    rules.layers.iter().position(|l| l == layer)
}

const CRATES_IO: &str = "registry+https://github.com/rust-lang/crates.io-index";

/// Files that would silently replace a protected configuration file.
const OVERRIDES: [&str; 8] = [
    ".clippy.toml",
    "rust-toolchain",
    ".cargo/config",
    ".cargo/deny.toml",
    ".deny.toml",
    "typos.toml",
    ".typos.toml",
    ".config/clippy.toml",
];

pub fn violations(model: &Model) -> Vec<String> {
    let rules = &model.rules;
    let mut out = Vec::new();
    let workspace: BTreeSet<&String> = model
        .packages
        .iter()
        .filter(|(_, p)| p.workspace)
        .map(|(n, _)| n)
        .collect();
    for name in &workspace {
        if !rules.crates.contains_key(name.as_str()) {
            out.push(format!(
                "[E-ARCH-UNKNOWN] {name} is not listed in architecture.toml"
            ));
        }
    }
    for path in &model.clippy_configs {
        out.push(format!(
            "[E-ARCH-OVERRIDE] {path} would take precedence over a protected configuration file; remove it"
        ));
    }
    for name in &model.patches {
        if !rules.allowed_patches.contains(name) {
            out.push(format!(
                "[E-ARCH-PATCH] Cargo.toml patches {name}; patches need an ADR and an architecture.toml entry"
            ));
        }
    }
    for (name, pin) in &rules.core_third_party {
        let Some(pkg) = model.packages.get(name) else {
            continue;
        };
        let features: BTreeSet<String> = pin.features.iter().cloned().collect();
        if pkg.version != pin.version
            || pkg.source.as_deref() != Some(CRATES_IO)
            || pkg.features != features
        {
            out.push(format!(
                "[E-ARCH-PIN] core dependency {name} resolved to {} from {:?} with features {:?}; \
                 architecture.toml pins {} from crates.io with {:?}",
                pkg.version, pkg.source, pkg.features, pin.version, features
            ));
        }
    }
    for name in rules.crates.keys() {
        if !workspace.contains(name) {
            out.push(format!(
                "[E-ARCH-UNKNOWN] architecture.toml lists {name}, which is not in the workspace"
            ));
        }
    }

    for (name, rule) in &rules.crates {
        let Some(pkg) = model.packages.get(name) else {
            continue;
        };
        let Some(layer) = layer_index(rules, &rule.layer) else {
            out.push(format!(
                "[E-ARCH-LAYER] {name} has unknown layer {}",
                rule.layer
            ));
            continue;
        };
        if !pkg.inherits_lints {
            out.push(format!(
                "[E-ARCH-LINTS] {name}/Cargo.toml must have `[lints] workspace = true`"
            ));
        }
        let is_game = name.starts_with("game-");
        if name.starts_with("game-") && name.ends_with("-sim") && !rule.core {
            out.push(format!(
                "[E-ARCH-CORE] {name} is a game sim and must be core = true"
            ));
        }
        for dep in &pkg.deps {
            if let Some(dep_rule) = rules.crates.get(dep) {
                let dep_layer = layer_index(rules, &dep_rule.layer).unwrap_or(usize::MAX);
                if dep_layer > layer {
                    out.push(format!(
                        "[E-ARCH-LAYER] {name} ({}) depends on {dep} ({}), a higher layer",
                        rule.layer, dep_rule.layer
                    ));
                } else if dep_layer == layer && !rule.same_layer.contains(dep) {
                    out.push(format!(
                        "[E-ARCH-SAME-LAYER] {name} depends on {dep} in its own layer {}; \
                         declare it in architecture.toml `same_layer`",
                        rule.layer
                    ));
                }
                if dep.starts_with("game-") && !is_game {
                    out.push(format!(
                        "[E-ARCH-GAME-DEP] engine crate {name} depends on game crate {dep}"
                    ));
                }
            }
            if let Some(wrapper) = rules.floor.get(dep)
                && wrapper != name
            {
                out.push(format!(
                    "[E-ARCH-FLOOR] {name} depends on {dep}; only {wrapper} may"
                ));
            }
        }
        if rule.core {
            check_core(model, name, pkg, &mut out);
        }
    }
    out
}

fn check_core(model: &Model, name: &str, pkg: &Package, out: &mut Vec<String>) {
    let rules = &model.rules;
    let attrs = pkg
        .lib_rs
        .as_deref()
        .map(crate::arch_scan::root_attributes)
        .unwrap_or_default();
    for required in ["#![no_std]", "#![forbid(unsafe_code)]"] {
        if !attrs.iter().any(|a| a == required) {
            out.push(format!(
                "[E-ARCH-ROOT-ATTR] {name}/src/lib.rs must start with {required} (core set)"
            ));
        }
    }
    if !model.overflow_checked.contains(name) {
        out.push(format!(
            "[E-ARCH-OVERFLOW] Cargo.toml needs [profile.release.package.{name}] overflow-checks = true"
        ));
    }
    if !pkg.build_deps.is_empty() {
        out.push(format!(
            "[E-ARCH-CORE-DEP] core crate {name} has build dependencies {:?}",
            pkg.build_deps
        ));
    }
    if let Some(path) = &pkg.odd_lib_path {
        out.push(format!(
            "[E-ARCH-ROOT-ATTR] core crate {name} builds its library from {path}; it must be src/lib.rs"
        ));
    }
    if pkg.has_build_script {
        out.push(format!(
            "[E-ARCH-BUILD] core crate {name} has a build script; the simulation build must not run code"
        ));
    }
    for (file, text) in &pkg.sources {
        for (line, message) in crate::arch_scan::scan(text) {
            out.push(format!(
                "[E-ARCH-ESCAPE] {file}:{line}: {message} (core crate {name})"
            ));
        }
    }
    // Transitive closure over normal dependencies.
    let mut seen = BTreeSet::new();
    let mut stack: Vec<(String, String)> = pkg
        .deps
        .iter()
        .map(|d| (d.clone(), name.to_string()))
        .collect();
    while let Some((dep, via)) = stack.pop() {
        if !seen.insert(dep.clone()) {
            continue;
        }
        let dep_rule = rules.crates.get(&dep);
        let allowed = match dep_rule {
            Some(r) if r.proc_macro => continue, // build-time only
            Some(r) => r.core,
            None => rules.core_third_party.contains_key(&dep),
        };
        if !allowed {
            out.push(format!(
                "[E-ARCH-CORE-DEP] core crate {name} reaches {dep} (via {via}), which is not in the core set"
            ));
            continue;
        }
        if let Some(p) = model.packages.get(&dep) {
            stack.extend(p.deps.iter().map(|d| (d.clone(), dep.clone())));
        }
    }
}

#[derive(Deserialize)]
struct Metadata {
    packages: Vec<MetaPackage>,
    workspace_members: Vec<String>,
    resolve: Resolve,
}

#[derive(Deserialize)]
struct MetaPackage {
    id: String,
    name: String,
    version: String,
    source: Option<String>,
    manifest_path: String,
    targets: Vec<MetaTarget>,
}

#[derive(Deserialize)]
struct MetaTarget {
    kind: Vec<String>,
    src_path: String,
}

#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    deps: Vec<NodeDep>,
    #[serde(default)]
    features: Vec<String>,
}

#[derive(Deserialize)]
struct NodeDep {
    pkg: String,
    dep_kinds: Vec<DepKind>,
}

#[derive(Deserialize)]
struct DepKind {
    kind: Option<String>,
}

pub fn load(root: &Path, locked: bool) -> Result<Model, String> {
    let text = fs::read_to_string(root.join("architecture.toml"))
        .map_err(|e| format!("architecture.toml: {e}"))?;
    let rules: ArchFile = toml::from_str(&text).map_err(|e| format!("architecture.toml: {e}"))?;
    let mut cmd = util::cargo(root);
    cmd.args(["metadata", "--format-version", "1"]);
    if locked {
        cmd.arg("--locked");
    }
    let json = util::run_quiet(root, "cargo-metadata", &mut cmd)?;
    let meta: Metadata =
        serde_json::from_str(&json).map_err(|e| format!("cargo metadata output: {e}"))?;
    let names: BTreeMap<&str, &MetaPackage> =
        meta.packages.iter().map(|p| (p.id.as_str(), p)).collect();
    let mut packages = BTreeMap::new();
    for node in &meta.resolve.nodes {
        let Some(meta_pkg) = names.get(node.id.as_str()) else {
            continue;
        };
        let mut pkg = Package {
            workspace: meta.workspace_members.contains(&node.id),
            version: meta_pkg.version.clone(),
            source: meta_pkg.source.clone(),
            features: node.features.iter().cloned().collect(),
            has_build_script: meta_pkg
                .targets
                .iter()
                .any(|t| t.kind.iter().any(|k| k == "custom-build")),
            ..Package::default()
        };
        for dep in &node.deps {
            let dep_name = names
                .get(dep.pkg.as_str())
                .map_or(dep.pkg.clone(), |p| p.name.clone());
            for kind in &dep.dep_kinds {
                match kind.kind.as_deref() {
                    None => {
                        pkg.deps.insert(dep_name.clone());
                    }
                    Some("build") => {
                        pkg.build_deps.insert(dep_name.clone());
                    }
                    _ => {}
                }
            }
        }
        if pkg.workspace {
            let dir = Path::new(&meta_pkg.manifest_path).parent().unwrap_or(root);
            pkg.lib_rs = fs::read_to_string(dir.join("src/lib.rs")).ok();
            let lib = meta_pkg
                .targets
                .iter()
                .find(|t| t.kind.iter().any(|k| k == "lib" || k == "proc-macro"));
            if let Some(lib) = lib
                && fs::canonicalize(&lib.src_path).ok()
                    != fs::canonicalize(dir.join("src/lib.rs")).ok()
            {
                pkg.odd_lib_path = Some(lib.src_path.clone());
            }
            // Every file, including directories named `target`, which a
            // `mod target;` can reach.
            for file in util::walk_all(&dir.join("src")) {
                if file.extension().is_some_and(|e| e == "rs") {
                    let text = fs::read_to_string(&file).unwrap_or_default();
                    pkg.sources.push((util::rel(root, &file), text));
                }
            }
            let manifest: toml::Table = fs::read_to_string(&meta_pkg.manifest_path)
                .ok()
                .and_then(|t| t.parse().ok())
                .unwrap_or_default();
            pkg.inherits_lints = manifest
                .get("lints")
                .and_then(|l| l.get("workspace"))
                .and_then(toml::Value::as_bool)
                == Some(true);
        }
        packages.insert(meta_pkg.name.clone(), pkg);
    }
    let root_manifest: toml::Table = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|e| format!("Cargo.toml: {e}"))?
        .parse()
        .map_err(|e| format!("Cargo.toml: {e}"))?;
    let overflow_checked = root_manifest
        .get("profile")
        .and_then(|p| p.get("release"))
        .and_then(|r| r.get("package"))
        .and_then(toml::Value::as_table)
        .map(|pkgs| {
            pkgs.iter()
                .filter(|(_, v)| {
                    v.get("overflow-checks").and_then(toml::Value::as_bool) == Some(true)
                })
                .map(|(k, _)| k.clone())
                .collect()
        })
        .unwrap_or_default();
    let mut patches: Vec<String> = root_manifest
        .get("patch")
        .and_then(toml::Value::as_table)
        .into_iter()
        .flat_map(|registries| registries.values())
        .filter_map(toml::Value::as_table)
        .flat_map(|crates| crates.keys().cloned())
        .collect();
    if let Some(replace) = root_manifest.get("replace").and_then(toml::Value::as_table) {
        patches.extend(replace.keys().cloned());
    }
    // Clippy reads the nearest clippy.toml or .clippy.toml above each crate;
    // only the root clippy.toml may exist.
    let mut clippy_configs: Vec<String> = util::walk(root)
        .into_iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n == "clippy.toml" || n == ".clippy.toml")
        })
        .map(|p| util::rel(root, &p))
        .filter(|rel| rel != "clippy.toml")
        .collect();
    // Files that take precedence over the protected configuration.
    for shadow in OVERRIDES {
        if root.join(shadow).exists() {
            clippy_configs.push(shadow.to_string());
        }
    }
    Ok(Model {
        rules,
        packages,
        overflow_checked,
        patches,
        clippy_configs,
    })
}

pub fn check(root: &Path) -> Result<String, String> {
    report(load(root, util::on_ci())?)
}

/// For self-tests on mutated copies, whose lock file may need updating.
pub fn check_unlocked(root: &Path) -> Result<String, String> {
    report(load(root, false)?)
}

fn report(model: Model) -> Result<String, String> {
    let found = violations(&model);
    if found.is_empty() {
        let n = model.rules.crates.len();
        Ok(format!("arch-check: {n} crates, no violations"))
    } else {
        Err(found.join("\n"))
    }
}

#[cfg(test)]
#[path = "arch_tests.rs"]
mod tests;
