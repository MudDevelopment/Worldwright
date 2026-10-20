use serde_json::{Value, json};
use worldwright_content::load_bytes;
#[test]
fn records_must_be_objects_not_positional_arrays() {
    let mut doc: Value =
        serde_json::from_str(include_str!("../../../examples/two-room/world.json")).unwrap();
    doc["rooms"][0] = json!(["reference:rooms.atrium", "Atrium", "Description", {"north":"reference:rooms.observatory"}, false]);
    assert_eq!(
        load_bytes("x", &serde_json::to_vec(&doc).unwrap(), Default::default())
            .unwrap_err()
            .diagnostics()[0]
            .code,
        "WWC_SCHEMA"
    );
    for bytes in [b"[1]".as_slice(), b"null", b"true"] {
        assert_eq!(
            load_bytes("x", bytes, Default::default())
                .unwrap_err()
                .diagnostics()[0]
                .code,
            "WWC_VERSION"
        );
    }
}
