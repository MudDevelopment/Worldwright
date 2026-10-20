use serde_json::{Value, json};
use worldwright_content::{ContentLimits, ContentReport, load_bytes};
fn example() -> Value {
    serde_json::from_str(include_str!("../../../examples/two-room/world.json")).unwrap()
}
fn load(value: &Value) -> Result<worldwright_content::ValidatedWorldContent, ContentReport> {
    load_bytes(
        "test",
        &serde_json::to_vec(value).unwrap(),
        Default::default(),
    )
}
fn has(value: &Value, code: &str) {
    let report = load(value).unwrap_err();
    assert!(
        report.diagnostics().iter().any(|d| d.code == code),
        "{report}"
    );
}

#[test]
fn identity_namespace_and_duplicates() {
    for (field, value, code) in [
        ("id", "Bad ID", "WWC_ID"),
        ("id", "other:rooms.atrium", "WWC_NAMESPACE"),
    ] {
        let mut doc = example();
        doc["rooms"][0][field] = json!(value);
        has(&doc, code);
    }
    let mut doc = example();
    doc["namespace"] = json!("UPPER");
    has(&doc, "WWC_NAMESPACE");
    let mut doc = example();
    doc["rooms"][1]["id"] = doc["rooms"][0]["id"].clone();
    has(&doc, "WWC_DUPLICATE_ROOM");
}
#[test]
fn text_and_unicode_byte_boundaries() {
    for field in ["title", "description"] {
        for text in [
            " ".into(),
            "line\nline".into(),
            "\u{1b}[31m".into(),
            "x".repeat(4097),
            "é".repeat(2049),
        ] {
            let mut doc = example();
            doc["rooms"][0][field] = json!(text);
            has(&doc, "WWC_TEXT");
        }
        let mut doc = example();
        doc["rooms"][0][field] = json!("é".repeat(2048));
        assert!(load(&doc).is_ok());
    }
}
#[test]
fn entry_and_exit_reference_errors() {
    let mut doc = example();
    doc["entry_room"] = json!("reference:rooms.missing");
    has(&doc, "WWC_UNKNOWN_ROOM");
    let mut doc = example();
    doc["rooms"][0]["exits"]["north"] = json!("reference:rooms.missing");
    has(&doc, "WWC_UNKNOWN_ROOM");
    let mut doc = example();
    doc["rooms"][0]["exits"]["north"] = json!("reference:rooms.atrium");
    has(&doc, "WWC_SELF_EXIT");
    let mut doc = example();
    doc["rooms"][0]["exits"]["east"] = json!("reference:rooms.observatory");
    has(&doc, "WWC_DUPLICATE_EXIT");
}
#[test]
fn exit_labels_and_pointer_escaping() {
    for label in ["", "NORTH", "a/b~c", &"x".repeat(65)] {
        let mut doc = example();
        doc["rooms"][0]["exits"] = json!({label: "reference:rooms.observatory"});
        has(&doc, "WWC_EXIT_LABEL");
        if label == "a/b~c" {
            assert!(
                load(&doc)
                    .unwrap_err()
                    .diagnostics()
                    .iter()
                    .any(|d| d.location.ends_with("a~1b~0c"))
            );
        }
    }
    let mut doc = example();
    doc["rooms"][0]["exits"] = json!({"x".repeat(64): "reference:rooms.observatory"});
    assert!(load(&doc).is_ok());
}
#[test]
fn one_way_is_valid_and_unreachable_requires_explicit_declaration() {
    let mut doc = example();
    doc["rooms"][1]["exits"] = json!({});
    assert!(load(&doc).is_ok());
    doc["rooms"][0]["exits"] = json!({});
    has(&doc, "WWC_UNREACHABLE");
    doc["rooms"][1]["allow_unreachable"] = json!(true);
    assert!(load(&doc).is_ok());
    doc["rooms"][1]["allow_unreachable"] = json!(false);
    has(&doc, "WWC_UNREACHABLE");
}
#[test]
fn bad_graph_does_not_produce_reachability_cascades() {
    let mut doc = example();
    doc["rooms"][0]["exits"]["north"] = json!("reference:rooms.missing");
    assert!(
        !load(&doc)
            .unwrap_err()
            .diagnostics()
            .iter()
            .any(|d| d.code == "WWC_UNREACHABLE")
    );
}
#[test]
fn diagnostic_cap_is_explicit_and_nonempty() {
    let mut doc = example();
    doc["rooms"][0]["title"] = json!("");
    doc["rooms"][1]["title"] = json!("");
    let report = load_bytes(
        "x",
        &serde_json::to_vec(&doc).unwrap(),
        ContentLimits {
            max_diagnostics: 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert_eq!(report.diagnostics().len(), 1);
    assert!(report.truncated());
    assert!(report.to_string().contains("omitted"));
    doc["rooms"][1]["title"] = json!("ok");
    let report = load_bytes(
        "x",
        &serde_json::to_vec(&doc).unwrap(),
        ContentLimits {
            max_diagnostics: 1,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(!report.truncated());
}
fn many_rooms(count: usize) -> Value {
    json!({"schema_version":1,"namespace":"n","entry_room":"n:r.room0", "rooms": (0..count).map(|i| json!({"id":format!("n:r.room{i}"),"title":"R","description":"D","exits":{},"allow_unreachable":true})).collect::<Vec<_>>()})
}
#[test]
fn room_capacity() {
    has(&many_rooms(0), "WWC_LIMIT");
    assert!(load(&many_rooms(1024)).is_ok());
    has(&many_rooms(1025), "WWC_LIMIT");
}
#[test]
fn exit_capacity() {
    let mut doc = many_rooms(66);
    doc["rooms"][0]["exits"] = Value::Object(
        (1..=64)
            .map(|i| (format!("e{i}"), json!(format!("n:r.room{i}"))))
            .collect(),
    );
    assert!(load(&doc).is_ok());
    doc["rooms"][0]["exits"]["e65"] = json!("n:r.room65");
    has(&doc, "WWC_LIMIT");
}
#[test]
fn maximum_room_text_and_count_fit_loader_budget() {
    let mut doc = many_rooms(1024);
    for room in doc["rooms"].as_array_mut().unwrap() {
        room["title"] = json!("x".repeat(4096));
        room["description"] = json!("y".repeat(4096));
    }
    assert!(load(&doc).is_ok());
}
