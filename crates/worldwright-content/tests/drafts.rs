use worldwright_content::{
    ContentLimits, load_bytes, load_draft_bytes, serialize_draft, validate_draft,
};

const VALID: &[u8] = br#"{
  "schema_version": 1,
  "namespace": "reference",
  "entry_room": "reference:rooms.start",
  "rooms": [{
    "id": "reference:rooms.start",
    "title": "Start",
    "description": "A room.",
    "exits": {}
  }]
}"#;

#[test]
fn draft_parse_serialize_reparse_is_equal() {
    let draft = load_draft_bytes("draft", VALID, ContentLimits::default()).unwrap();
    let bytes = serialize_draft("draft", &draft, ContentLimits::default()).unwrap();
    let reparsed = load_draft_bytes("draft", &bytes, ContentLimits::default()).unwrap();
    assert_eq!(draft, reparsed);
}

#[test]
fn draft_unknown_field_is_rejected() {
    let mut value: serde_json::Value = serde_json::from_slice(VALID).unwrap();
    value["unknown"] = serde_json::json!(true);
    let bytes = serde_json::to_vec(&value).unwrap();
    assert!(load_draft_bytes("draft", &bytes, ContentLimits::default()).is_err());
}

#[test]
fn draft_semantic_diagnostics_match_authoritative_loader() {
    let mut value: serde_json::Value = serde_json::from_slice(VALID).unwrap();
    value["rooms"][0]["exits"] = serde_json::json!({"north":"reference:rooms:missing"});
    let bytes = serde_json::to_vec(&value).unwrap();
    let draft = load_draft_bytes("draft", &bytes, ContentLimits::default()).unwrap();
    let draft_error = validate_draft("draft", draft, ContentLimits::default()).unwrap_err();
    let loaded_error = load_bytes("draft", &bytes, ContentLimits::default()).unwrap_err();
    let project = |report: &worldwright_content::ContentReport| {
        report
            .diagnostics()
            .iter()
            .map(|d| (d.code, d.location.clone(), d.message.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(project(&draft_error), project(&loaded_error));
}
