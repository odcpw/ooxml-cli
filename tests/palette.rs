#[path = "../src/palette.rs"]
mod palette;

use palette::{BUILT_IN_MASTER_TEXT_PAIRINGS, MasterTextClass, Srgb, ThemePalette};

#[test]
fn every_built_in_master_pairing_meets_wcag_for_1024_random_seeds() {
    assert_eq!(
        BUILT_IN_MASTER_TEXT_PAIRINGS
            .iter()
            .filter(|pairing| pairing.text_class == MasterTextClass::Body)
            .count(),
        6
    );
    assert_eq!(
        BUILT_IN_MASTER_TEXT_PAIRINGS
            .iter()
            .filter(|pairing| pairing.text_class == MasterTextClass::Large)
            .count(),
        8
    );
    let mut state = 0xA5C3_1F27_u32;
    for sample in 0..1024 {
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        let seed = Srgb::new((state >> 16) as u8, (state >> 8) as u8, state as u8);
        let derived = ThemePalette::from_seed(seed);
        assert_eq!(derived, ThemePalette::from_seed(seed), "seed #{sample}");
        for pairing in BUILT_IN_MASTER_TEXT_PAIRINGS {
            let actual = derived.contrast_for(*pairing);
            let minimum_contrast = match pairing.text_class {
                MasterTextClass::Body => 4.5,
                MasterTextClass::Large => 3.0,
            };
            assert!(
                actual >= minimum_contrast,
                "seed {} pairing {:?}/{:?}: {actual:.6} < {:.1}",
                seed.to_hex(),
                pairing.foreground,
                pairing.background,
                minimum_contrast,
            );
        }
    }
}

#[test]
fn random_srgb_colors_roundtrip_through_oklch_for_1024_samples() {
    let mut state = 0x6D2B_79F5_u32;
    for sample in 0..1024 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let original = Srgb::new((state >> 16) as u8, (state >> 8) as u8, state as u8);
        assert_eq!(
            original.to_oklch().to_srgb(),
            original,
            "roundtrip sample {sample}: {}",
            original.to_hex()
        );
    }
}

#[test]
fn four_seed_palettes_match_reviewed_goldens() {
    let goldens = [
        (
            "1F4E79",
            r#"{"dk1":"0A1016","lt1":"FFFFFF","dk2":"1E3246","lt2":"EAF1F9","accent1":"1F4E79","accent2":"6F5991","accent3":"7F4557","accent4":"87572A","accent5":"505D28","accent6":"097063","hlink":"72630A","folHlink":"1A6C60"}"#,
        ),
        (
            "C00000",
            r#"{"dk1":"190B09","lt1":"FFFFFF","dk2":"521D17","lt2":"FFEBE8","accent1":"C00000","accent2":"8E6600","accent3":"1D750D","accent4":"007A80","accent5":"2A59B0","accent6":"9141A0","hlink":"006A9C","folHlink":"803E8C"}"#,
        ),
        (
            "70AD47",
            r#"{"dk1":"0B1207","lt1":"FFFFFF","dk2":"213812","lt2":"EBF3E6","accent1":"67A33D","accent2":"00A59A","accent3":"3A87C6","accent4":"A175CA","accent5":"B85F73","accent6":"BC7720","hlink":"9B3877","folHlink":"854F00"}"#,
        ),
        (
            "808080",
            r#"{"dk1":"0A1016","lt1":"FFFFFF","dk2":"1E3246","lt2":"EAF1F9","accent1":"808080","accent2":"937CB7","accent3":"A46778","accent4":"AC7A4C","accent5":"71804B","accent6":"3D9487","hlink":"71640B","folHlink":"196C61"}"#,
        ),
    ];
    for (seed, expected) in goldens {
        let palette = ThemePalette::derive(seed).expect("golden seed");
        assert_eq!(
            serde_json::to_string(&palette).expect("serialize golden palette"),
            expected,
            "golden seed {seed}"
        );
    }
}
