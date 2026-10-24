//! Unit tests for arch-check rules on synthetic workspaces.

use super::*;

const RULES: &str = r#"
version = 1
layers = ["L0", "L1", "L2"]
[core_third_party]
libm = { version = "0.2.16", features = [] }
[floor]
wgpu = "eng-gpu"
[crates.eng-derive]
layer = "L0"
proc_macro = true
[crates.eng-a]
layer = "L0"
core = true
same_layer = ["eng-derive"]
[crates.eng-b]
layer = "L1"
core = true
[crates.eng-std]
layer = "L1"
[crates.eng-gpu]
layer = "L2"
"#;

const CORE_LIB: &str = "//! Doc.\n#![no_std]\n#![forbid(unsafe_code)]\n\npub fn f() {}\n";

fn pkg(deps: &[&str], lib: Option<&str>) -> Package {
    Package {
        workspace: true,
        deps: deps.iter().map(|d| d.to_string()).collect(),
        lib_rs: lib.map(String::from),
        sources: lib
            .map(|l| vec![("src/lib.rs".into(), l.into())])
            .unwrap_or_default(),
        inherits_lints: true,
        ..Package::default()
    }
}

fn model(edit: impl FnOnce(&mut BTreeMap<String, Package>)) -> Model {
    let mut packages = BTreeMap::new();
    packages.insert("eng-derive".into(), pkg(&["syn"], Some("pub fn d() {}")));
    packages.insert("eng-a".into(), pkg(&["eng-derive", "libm"], Some(CORE_LIB)));
    packages.insert("eng-b".into(), pkg(&["eng-a"], Some(CORE_LIB)));
    packages.insert(
        "eng-std".into(),
        pkg(&["eng-a", "serde"], Some("pub fn s() {}")),
    );
    packages.insert("eng-gpu".into(), pkg(&["wgpu"], Some("pub fn g() {}")));
    for third in ["syn", "libm", "serde", "wgpu"] {
        packages.insert(
            third.into(),
            Package {
                version: "0.2.16".into(),
                source: Some(CRATES_IO.into()),
                ..Package::default()
            },
        );
    }
    edit(&mut packages);
    Model {
        rules: toml::from_str(RULES).unwrap(),
        packages,
        overflow_checked: ["eng-a", "eng-b"].map(String::from).into(),
        patches: Vec::new(),
        clippy_configs: Vec::new(),
    }
}

fn tags(m: &Model) -> Vec<String> {
    violations(m)
        .iter()
        .map(|v| {
            v.split(']')
                .next()
                .unwrap()
                .trim_start_matches('[')
                .to_string()
        })
        .collect()
}

#[test]
fn clean_model_passes() {
    assert_eq!(violations(&model(|_| {})), Vec::<String>::new());
}

#[test]
fn core_crate_reaching_a_std_crate_fails() {
    let m = model(|p| {
        p.get_mut("eng-b").unwrap().deps.insert("eng-std".into());
    });
    assert!(
        tags(&m).contains(&"E-ARCH-CORE-DEP".into()),
        "{:?}",
        violations(&m)
    );
}

#[test]
fn core_crate_reaching_third_party_transitively_fails() {
    let m = model(|p| {
        p.get_mut("libm").unwrap().deps.insert("serde".into());
    });
    let v = violations(&m);
    assert!(
        v.iter().any(|s| s.contains("reaches serde (via libm)")),
        "{v:?}"
    );
}

#[test]
fn proc_macro_dependencies_are_build_time() {
    // eng-derive -> syn is fine for a core crate using eng-derive.
    assert!(violations(&model(|_| {})).is_empty());
}

#[test]
fn missing_no_std_fails() {
    let m = model(|p| {
        let lib = CORE_LIB.replace("#![no_std]\n", "");
        p.get_mut("eng-a").unwrap().lib_rs = Some(lib);
    });
    assert_eq!(tags(&m), ["E-ARCH-ROOT-ATTR"]);
    let m = model(|p| {
        let lib = CORE_LIB.replace("#![no_std]", "#![cfg_attr(not(test), no_std)]");
        p.get_mut("eng-a").unwrap().lib_rs = Some(lib);
    });
    assert_eq!(
        tags(&m),
        ["E-ARCH-ROOT-ATTR"],
        "conditional no_std is not enough"
    );
}

#[test]
fn layering_and_same_layer_and_floor() {
    let m = model(|p| {
        p.get_mut("eng-a").unwrap().deps.insert("eng-b".into());
    });
    assert!(tags(&m).contains(&"E-ARCH-LAYER".into()));
    let m = model(|p| {
        p.get_mut("eng-std").unwrap().deps.insert("eng-b".into());
    });
    assert_eq!(tags(&m), ["E-ARCH-SAME-LAYER"]);
    let m = model(|p| {
        p.get_mut("eng-std").unwrap().deps.insert("wgpu".into());
    });
    assert_eq!(tags(&m), ["E-ARCH-FLOOR"]);
}

#[test]
fn escapes_lints_overflow_and_unknown() {
    let m = model(|p| {
        let a = p.get_mut("eng-a").unwrap();
        a.sources.push((
            "src/x.rs".into(),
            "#[allow(clippy::disallowed_methods)]\nfn x() {}".into(),
        ));
    });
    assert_eq!(tags(&m), ["E-ARCH-ESCAPE"]);
    let m = model(|p| p.get_mut("eng-std").unwrap().inherits_lints = false);
    assert_eq!(tags(&m), ["E-ARCH-LINTS"]);
    let mut m = model(|_| {});
    m.overflow_checked.remove("eng-b");
    assert_eq!(tags(&m), ["E-ARCH-OVERFLOW"]);
    let m = model(|p| {
        p.insert("eng-new".into(), pkg(&[], None));
    });
    assert_eq!(tags(&m), ["E-ARCH-UNKNOWN"]);
}

#[test]
fn root_attributes_skip_comments() {
    let attrs = crate::arch_scan::root_attributes(
        "//! doc\n\n// note\n#![no_std]\n#![forbid(unsafe_code)]\nuse x;\n#![later]\n",
    );
    assert_eq!(attrs, ["#![no_std]", "#![forbid(unsafe_code)]"]);
}

#[test]
fn core_sources_cannot_escape() {
    for code in [
        "extern crate std;",
        "#[allow(\n    clippy::disallowed_methods\n)]\nfn x() {}",
        "#[allow(clippy::style)]\nfn x() {}",
        "#[path = \"../../elsewhere.rs\"]\nmod m;",
    ] {
        let m = model(|p| {
            let a = p.get_mut("eng-a").unwrap();
            a.sources.push(("src/x.rs".into(), code.into()));
        });
        assert_eq!(tags(&m), ["E-ARCH-ESCAPE"], "{code:?}");
    }
}

#[test]
fn build_scripts_clippy_configs_patches_and_pins() {
    let m = model(|p| p.get_mut("eng-a").unwrap().has_build_script = true);
    assert_eq!(tags(&m), ["E-ARCH-BUILD"]);
    let mut m = model(|_| {});
    m.clippy_configs.push("crates/eng-a/clippy.toml".into());
    assert_eq!(tags(&m), ["E-ARCH-OVERRIDE"]);
    let m = model(|p| p.get_mut("eng-a").unwrap().odd_lib_path = Some("real/lib.rs".into()));
    assert_eq!(
        tags(&m),
        ["E-ARCH-ROOT-ATTR"],
        "a [lib] path override skips the src/lib.rs check"
    );
    let mut m = model(|_| {});
    m.patches.push("libm".into());
    assert_eq!(tags(&m), ["E-ARCH-PATCH"]);
    let m = model(|p| {
        p.get_mut("libm").unwrap().features.insert("arch".into());
    });
    assert_eq!(
        tags(&m),
        ["E-ARCH-PIN"],
        "the arch feature changes libm's code paths"
    );
    let m = model(|p| p.get_mut("libm").unwrap().version = "0.2.17".into());
    assert_eq!(tags(&m), ["E-ARCH-PIN"]);
    let m =
        model(|p| p.get_mut("libm").unwrap().source = Some("git+https://example.com/libm".into()));
    assert_eq!(tags(&m), ["E-ARCH-PIN"]);
}
