use hangar_lib::sidecar::{slugify, Sidecar};

#[test]
fn slugify_basics() {
    assert_eq!(slugify("Verdant Pro V1"), "verdant-pro-v1");
    assert_eq!(slugify("  BasedBoard!! "), "basedboard");
    assert_eq!(slugify("---"), "project");
    assert_eq!(slugify("ESP32_dev board"), "esp32-dev-board");
}

#[test]
fn sidecar_roundtrip() {
    let dir = std::env::temp_dir().join(format!("hangar-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let sc = Sidecar::new("Test Project");
    sc.save(&dir).unwrap();

    let loaded = Sidecar::load(&dir).expect("sidecar should load");
    assert_eq!(loaded.name, "Test Project");
    assert_eq!(loaded.status, "active");
    assert_eq!(loaded.progress, 0);
    // Deterministic color: same name → same color.
    assert_eq!(loaded.color, Sidecar::new("Test Project").color);

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn sidecar_load_or_init_creates_file() {
    let dir = std::env::temp_dir().join(format!("hangar-test-init-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    assert!(Sidecar::load(&dir).is_none());
    let sc = Sidecar::load_or_init(&dir, "Fresh").unwrap();
    assert_eq!(sc.name, "Fresh");
    assert!(dir.join(".hangar/project.json").exists());

    std::fs::remove_dir_all(&dir).unwrap();
}
