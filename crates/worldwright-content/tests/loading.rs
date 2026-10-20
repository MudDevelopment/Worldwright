use worldwright_content::{ContentLimits, load_bytes, load_file};
const EXAMPLE: &[u8] = include_bytes!("../../../examples/two-room/world.json");
fn error(bytes: &[u8]) -> worldwright_content::ContentReport {
    load_bytes("test", bytes, ContentLimits::default()).unwrap_err()
}
fn code(bytes: &[u8], expected: &str) {
    assert_eq!(error(bytes).diagnostics()[0].code, expected);
}

#[test]
fn reference_file_loads() {
    let world = load_bytes("example", EXAMPLE, ContentLimits::default()).unwrap();
    assert_eq!(world.rooms().len(), 2);
    assert_eq!(world.exit_count(), 2);
    assert_eq!(world.namespace(), "reference");
}
#[test]
fn malformed_encoding_syntax_and_trailing_data() {
    for bytes in [
        b"{".as_slice(),
        b"\xff",
        b"{}{}",
        b"// comment\n{}",
        b"[1,]",
    ] {
        code(bytes, "WWC_JSON");
    }
    let mut bytes = EXAMPLE.to_vec();
    bytes.extend_from_slice(b" trailing");
    code(&bytes, "WWC_JSON");
}
#[test]
fn version_dispatch_is_independent_of_v1_shape() {
    for bytes in [
        b"{}".as_slice(),
        b"{\"schema_version\":2,\"future\":true}",
        b"{\"schema_version\":null}",
        b"{\"schema_version\":\"1\"}",
        b"{\"schema_version\":1.0}",
        b"{\"schema_version\":-1}",
    ] {
        code(bytes, "WWC_VERSION");
    }
}
#[test]
fn duplicate_keys_are_never_overwritten() {
    for bytes in [
        b"{\"schema_version\":1,\"schema_version\":1}".as_slice(),
        b"{\"x\":1,\"\\u0078\":2}",
        b"{\"nested\":{\"a\":1,\"a\":2}}",
    ] {
        code(bytes, "WWC_DUPLICATE_KEY");
    }
    let source = String::from_utf8(EXAMPLE.to_vec()).unwrap().replace(
        "\"north\": \"reference:rooms.observatory\"",
        "\"north\": \"reference:rooms.observatory\", \"north\": \"reference:rooms.atrium\"",
    );
    code(source.as_bytes(), "WWC_DUPLICATE_KEY");
}
#[test]
fn strict_fields_and_types() {
    let value: serde_json::Value = serde_json::from_slice(EXAMPLE).unwrap();
    for pointer in [
        "/rooms/0/title",
        "/entry_room",
        "/rooms/0/allow_unreachable",
    ] {
        let mut v = value.clone();
        if pointer.ends_with("allow_unreachable") {
            v["rooms"][0]["allow_unreachable"] = serde_json::json!("true");
        } else {
            *v.pointer_mut(pointer).unwrap() = serde_json::json!(42);
        }
        code(&serde_json::to_vec(&v).unwrap(), "WWC_SCHEMA");
    }
    for root in [true, false] {
        let mut v = value.clone();
        if root {
            v["typo"] = serde_json::json!(1);
        } else {
            v["rooms"][0]["typo"] = serde_json::json!(1);
        }
        code(&serde_json::to_vec(&v).unwrap(), "WWC_SCHEMA");
    }
    let source = std::str::from_utf8(EXAMPLE)
        .unwrap()
        .replace("\"title\": \"Atrium\",", "");
    code(source.as_bytes(), "WWC_SCHEMA");
}
#[test]
fn byte_limits_are_exact_and_cannot_be_relaxed() {
    let limits = ContentLimits {
        max_bytes: EXAMPLE.len(),
        ..Default::default()
    };
    assert!(load_bytes("x", EXAMPLE, limits).is_ok());
    assert!(
        load_bytes(
            "x",
            EXAMPLE,
            ContentLimits {
                max_bytes: EXAMPLE.len() - 1,
                ..limits
            }
        )
        .is_err()
    );
    for limits in [
        ContentLimits {
            max_bytes: 0,
            ..Default::default()
        },
        ContentLimits {
            max_depth: 33,
            ..Default::default()
        },
        ContentLimits {
            max_diagnostics: 101,
            ..Default::default()
        },
    ] {
        assert_eq!(
            load_bytes("x", EXAMPLE, limits).unwrap_err().diagnostics()[0].code,
            "WWC_LIMIT"
        );
    }
}
#[test]
fn nesting_including_unknown_data_is_bounded() {
    let source = format!("{}0{}", "[".repeat(32), "]".repeat(32));
    code(source.as_bytes(), "WWC_VERSION"); // guard passed; root is not a document
    let source = format!("{}0{}", "[".repeat(33), "]".repeat(33));
    code(source.as_bytes(), "WWC_LIMIT");
    let mut value: serde_json::Value = serde_json::from_slice(EXAMPLE).unwrap();
    value["rooms"][0]["title"] = serde_json::json!("literal [[[[ {{{{ \\\" braces");
    assert!(
        load_bytes(
            "x",
            &serde_json::to_vec(&value).unwrap(),
            Default::default()
        )
        .is_ok()
    );
}
#[test]
fn syntax_diagnostics_have_locations_and_safe_display() {
    let report = load_bytes("bad\n\u{1b}[31m.json", b"{", Default::default()).unwrap_err();
    let display = report.to_string();
    assert!(display.contains("line 1"));
    assert!(display.contains("\\u{1b}"));
    assert!(!display.contains('\u{1b}'));
    assert_eq!(report.source_name(), "bad\n\u{1b}[31m.json");
}
#[test]
fn file_errors_are_typed() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in [root.to_owned(), root.join("no-such-world-content-991.json")] {
        assert_eq!(
            load_file(path, Default::default())
                .unwrap_err()
                .diagnostics()[0]
                .code,
            "WWC_IO"
        );
    }
    assert!(
        load_file(
            root.join("../../examples/two-room/world.json"),
            Default::default()
        )
        .is_ok()
    );
}
#[test]
fn parser_container_and_total_value_budgets() {
    code(
        format!("[{}]", vec!["0"; 1025].join(",")).as_bytes(),
        "WWC_LIMIT",
    );
    let child = format!("[{}]", vec!["0"; 1000].join(","));
    code(
        format!("[{}]", vec![child; 201].join(",")).as_bytes(),
        "WWC_LIMIT",
    );
}
