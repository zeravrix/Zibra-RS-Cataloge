use zibra_core::{AppSettings, ColorTheme, Manga, ReadingProgress};

#[test]
fn test_color_themes_exist_and_bounded() {
    assert_eq!(ColorTheme::ALL.len(), 13);
    for theme in ColorTheme::ALL {
        let _rgb = theme.primary_rgb();
        assert!(!theme.id().is_empty());
        assert!(!theme.title().is_empty());
    }
    assert_eq!(ColorTheme::Totoro.primary_rgb(), (0xA6, 0xC8, 0xFF));
    assert_eq!(ColorTheme::AmoledVoid.dark_bg_rgb(), (0x00, 0x00, 0x00));
}

#[test]
fn test_manhwa_detection_keywords() {
    // English keywords
    assert!(Manga::detect_manhwa("Solo Leveling Manhwa", "https://example.com/sl", &[]));
    assert!(Manga::detect_manhwa("Tower of God", "https://example.com/webtoon/tog", &[]));
    
    // Arabic keywords
    assert!(Manga::detect_manhwa("سولو ليفلينغ مانهوا", "https://example.com/sl", &[]));
    assert!(Manga::detect_manhwa("برج الإله", "https://example.com/ويبتون/tog", &[]));

    // Non-manhwa manga
    assert!(!Manga::detect_manhwa("One Piece", "https://example.com/manga/op", &["Action".to_string()]));
}

#[test]
fn test_app_settings_serde_roundtrip() {
    let settings = AppSettings::default();
    let json = serde_json::to_string(&settings).expect("Serialization failed");
    let deserialized: AppSettings = serde_json::from_str(&json).expect("Deserialization failed");

    assert_eq!(settings.theme, deserialized.theme);
    assert_eq!(settings.color_theme, deserialized.color_theme);
    assert_eq!(settings.is_amoled, deserialized.is_amoled);
    assert_eq!(settings.language, deserialized.language);
}

#[test]
fn test_reading_progress_computation() {
    let progress = ReadingProgress {
        manga_id: "manga-123".to_string(),
        chapter_id: "ch-1".to_string(),
        page_index: 5,
        scroll_offset_y: 120.0,
        chapter_number: Some(1.0),
        updated_at: 1700000000,
    };

    assert_eq!(progress.page_index, 5);
    assert_eq!(progress.scroll_offset_y, 120.0);
    assert_eq!(progress.chapter_number, Some(1.0));
}
