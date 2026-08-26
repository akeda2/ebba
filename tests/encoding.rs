use ebba::document::encoding::{
    ConfirmationReason, ContentOverride, DetectedEncoding, DetectionOptions, StartupContentMode,
    StartupDecision, StartupPayload, detect_startup_mode,
};
use ebba::document::format::{LineEndingIndicator, LineEndingMode};

#[test]
fn detects_utf8_and_bom_encoded_text() {
    let utf8 = b"hello\nworld";
    let plain = detect_startup_mode(utf8, DetectionOptions::default());
    match plain {
        StartupDecision::Ready(plan) => {
            assert_eq!(plan.mode, StartupContentMode::DecodedText);
            assert_eq!(plan.encoding, DetectedEncoding::Utf8);
            assert_eq!(
                plan.line_endings.detected(),
                Some(ebba::document::format::LineEnding::Lf)
            );
        }
        other => panic!("expected ready plan, got {other:?}"),
    }

    let utf8_bom = [0xEF, 0xBB, 0xBF, b'a', b'\n'];
    let bom = detect_startup_mode(&utf8_bom, DetectionOptions::default());
    match bom {
        StartupDecision::Ready(plan) => {
            assert_eq!(plan.mode, StartupContentMode::DecodedText);
            assert_eq!(plan.encoding, DetectedEncoding::Utf8Bom);
            match plan.payload {
                StartupPayload::DecodedText {
                    text,
                    strip_utf8_bom,
                } => {
                    assert_eq!(text, "a\n");
                    assert!(strip_utf8_bom);
                }
                other => panic!("expected decoded text, got {other:?}"),
            }
        }
        other => panic!("expected ready plan, got {other:?}"),
    }
}

#[test]
fn classifies_binary_data_and_honors_overrides() {
    let binary = [0x41, 0x00, 0x42, 0x43];
    let auto = detect_startup_mode(&binary, DetectionOptions::default());
    match auto {
        StartupDecision::Ready(plan) => {
            assert_eq!(plan.mode, StartupContentMode::HexReadOnly);
        }
        other => panic!("expected ready plan, got {other:?}"),
    }

    let forced_text = detect_startup_mode(
        &binary,
        DetectionOptions {
            content_override: ContentOverride::Text,
            ..DetectionOptions::default()
        },
    );
    match forced_text {
        StartupDecision::Ready(plan) => {
            assert_eq!(plan.mode, StartupContentMode::BytePreservingFallbackText);
        }
        other => panic!("expected ready plan, got {other:?}"),
    }

    let plain_text = b"just text";
    let forced_binary = detect_startup_mode(
        plain_text,
        DetectionOptions {
            content_override: ContentOverride::Binary,
            ..DetectionOptions::default()
        },
    );
    match forced_binary {
        StartupDecision::Ready(plan) => {
            assert_eq!(plan.mode, StartupContentMode::HexReadOnly);
        }
        other => panic!("expected ready plan, got {other:?}"),
    }
}

#[test]
fn falls_back_for_uncertain_non_utf8_text() {
    let uncertain = [0xC3, 0x28, b'a', b'\n'];
    let decision = detect_startup_mode(&uncertain, DetectionOptions::default());
    match decision {
        StartupDecision::Ready(plan) => {
            assert_eq!(plan.mode, StartupContentMode::BytePreservingFallbackText);
            assert_eq!(plan.encoding, DetectedEncoding::Unknown8Bit);
        }
        other => panic!("expected fallback plan, got {other:?}"),
    }
}

#[test]
fn preserves_or_overrides_line_ending_metadata() {
    let mixed = b"a\r\nb\n";
    let preserve = detect_startup_mode(
        mixed,
        DetectionOptions {
            line_ending_mode: LineEndingMode::Preserve,
            ..DetectionOptions::default()
        },
    );
    let preserve_plan = match preserve {
        StartupDecision::Ready(plan) => plan,
        other => panic!("expected ready plan, got {other:?}"),
    };
    assert!(preserve_plan.line_endings.has_mixed_endings());
    assert_eq!(
        preserve_plan.line_endings.effective_for_save(),
        Some(ebba::document::format::LineEnding::Crlf)
    );

    let forced_lf = detect_startup_mode(
        mixed,
        DetectionOptions {
            line_ending_mode: LineEndingMode::Lf,
            ..DetectionOptions::default()
        },
    );
    let forced_lf_plan = match forced_lf {
        StartupDecision::Ready(plan) => plan,
        other => panic!("expected ready plan, got {other:?}"),
    };
    assert_eq!(
        forced_lf_plan.line_endings.effective_for_save(),
        Some(ebba::document::format::LineEnding::Lf)
    );
}

#[test]
fn exposes_confirmation_for_large_or_non_resynchronizable_loads() {
    let utf16le = [0xFF, 0xFE, b'h', 0x00, b'i', 0x00];
    let utf16 = detect_startup_mode(
        &utf16le,
        DetectionOptions {
            large_file_threshold_bytes: None,
            ..DetectionOptions::default()
        },
    );

    match utf16 {
        StartupDecision::RequiresConfirmation(required) => {
            assert!(
                required
                    .reasons
                    .contains(&ConfirmationReason::NonResynchronizableEncoding {
                        encoding: DetectedEncoding::Utf16LeBom
                    })
            );
        }
        other => panic!("expected confirmation, got {other:?}"),
    }

    let small_text = b"0123456789";
    let large_policy = detect_startup_mode(
        small_text,
        DetectionOptions {
            large_file_threshold_bytes: Some(4),
            confirm_non_resynchronizable_encoding: false,
            ..DetectionOptions::default()
        },
    );
    match large_policy {
        StartupDecision::RequiresConfirmation(required) => {
            assert!(required.reasons.contains(&ConfirmationReason::LargeFile {
                size_bytes: 10,
                threshold_bytes: 4,
            }));
        }
        other => panic!("expected large-file confirmation, got {other:?}"),
    }
}

#[test]
fn detects_utf16be_and_decodes_text() {
    let utf16be = [0xFE, 0xFF, 0x00, b'h', 0x00, b'i', 0x00, b'\n'];
    let decision = detect_startup_mode(
        &utf16be,
        DetectionOptions {
            confirm_non_resynchronizable_encoding: false,
            ..DetectionOptions::default()
        },
    );

    match decision {
        StartupDecision::Ready(plan) => {
            assert_eq!(plan.mode, StartupContentMode::DecodedText);
            assert_eq!(plan.encoding, DetectedEncoding::Utf16BeBom);
            match plan.payload {
                StartupPayload::DecodedText { text, .. } => assert_eq!(text, "hi\n"),
                other => panic!("expected decoded text payload, got {other:?}"),
            }
        }
        other => panic!("expected ready plan, got {other:?}"),
    }
}

#[test]
fn malformed_utf16_bom_falls_back_to_byte_preserving_text() {
    let malformed = [0xFF, 0xFE, 0x61];
    let decision = detect_startup_mode(&malformed, DetectionOptions::default());
    match decision {
        StartupDecision::Ready(plan) => {
            assert_eq!(plan.mode, StartupContentMode::BytePreservingFallbackText);
            assert_eq!(plan.encoding, DetectedEncoding::Unknown8Bit);
        }
        other => panic!("expected fallback plan, got {other:?}"),
    }
}

#[test]
fn detects_bomless_utf16le_text_conservatively() {
    let bomless_utf16le = [b'h', 0x00, b'e', 0x00, b'l', 0x00, b'l', 0x00, b'o', 0x00];
    let decision = detect_startup_mode(
        &bomless_utf16le,
        DetectionOptions {
            confirm_non_resynchronizable_encoding: false,
            ..DetectionOptions::default()
        },
    );

    match decision {
        StartupDecision::Ready(plan) => {
            assert_eq!(plan.mode, StartupContentMode::DecodedText);
            assert_eq!(plan.encoding, DetectedEncoding::Utf16Le);
            match plan.payload {
                StartupPayload::DecodedText { text, .. } => assert_eq!(text, "hello"),
                other => panic!("expected decoded text payload, got {other:?}"),
            }
        }
        other => panic!("expected ready plan, got {other:?}"),
    }
}

#[test]
fn line_ending_indicator_supports_cr_and_none() {
    let cr_only = detect_startup_mode(b"a\rb\r", DetectionOptions::default());
    let cr_plan = match cr_only {
        StartupDecision::Ready(plan) => plan,
        other => panic!("expected ready plan, got {other:?}"),
    };
    assert_eq!(cr_plan.line_endings.indicator(), LineEndingIndicator::Cr);

    let none = detect_startup_mode(b"abc", DetectionOptions::default());
    let none_plan = match none {
        StartupDecision::Ready(plan) => plan,
        other => panic!("expected ready plan, got {other:?}"),
    };
    assert_eq!(none_plan.line_endings.indicator(), LineEndingIndicator::None);
}
