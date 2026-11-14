use zibra_sources::LiveSourceRegistry;

#[test]
fn test_live_source_registry_pure_rust_all_sources() {
    let registry = LiveSourceRegistry::new();
    let total = registry.all_sources().len();
    println!("Total pure Rust sources registered: {}", total);
    assert!(
        total >= 1000,
        "LiveSourceRegistry must contain at least 1,000 pure Rust sources, got {}",
        total
    );

    // Verify key sources exist
    assert!(registry.get("MANGADEX").is_some(), "MangaDex must exist");
    assert!(registry.get("MANGASWAT").is_some(), "MangaSwat must exist");
    assert!(registry.get("AZORA").is_some(), "Azora must exist");
    assert!(registry.get("TEAMX").is_some(), "TeamX must exist");
    assert!(registry.get("TOONILY").is_some(), "Toonily must exist");
    assert!(registry.get("ASURASCANS").is_some(), "AsuraScans must exist");
    assert!(registry.get("kotatsu:COMIC21").is_some() || registry.get("kotatsu:HARIMANGA").is_some(), "Kotatsu catalog sources must exist");
}
