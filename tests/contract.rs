// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors

//! The contract suite: builds this crate as an `.oxp` file and checks it
//! through OxiGuitar's own plugin host, as the app would load it. `cargo
//! test` runs it with the rest.

use std::path::Path;

#[test]
fn the_plugin_file_keeps_the_contract() {
    let crate_folder = Path::new(env!("CARGO_MANIFEST_DIR"));
    let oxp =
        oxiguitar_plugin_test::build(crate_folder, env!("CARGO_PKG_NAME"), "tuxguitar", false)
            .expect("the .oxp file builds");
    let report = oxiguitar_plugin_test::check(&oxp);
    for passed in &report.passed {
        println!("ok    {passed}");
    }
    assert!(report.ok(), "failed: {:#?}", report.failed);
}

#[test]
fn its_signature_recognises_tuxguitar_2_files_only() {
    use oxiguitar_plugin::api::manifest::Manifest;
    use oxiguitar_plugin::api::signature::Sniff;
    let manifest: Manifest =
        serde_json::from_str(include_str!("../manifest.json")).expect("the manifest reads");
    let recognises = |name: &str| {
        let data = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("fixtures")
                .join(name),
        )
        .unwrap();
        let sniff = Sniff::new(&data);
        manifest.formats[0]
            .signatures
            .iter()
            .any(|s| s.matches(&sniff))
    };
    assert!(recognises("oxiguitar-test.tg"));
    assert!(recognises("tuxguitar/reference_20.tg"));
    // TuxGuitar 1.x's binary form is not claimed, nor a ZIP without the
    // version file.
    assert!(!recognises("tuxguitar/reference_15.tg"));
    assert!(!recognises("tuxguitar/noVersion.tg"));
}
