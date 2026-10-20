use serde_json::{Value, json};
use worldwright_content::{instantiate, load_bytes};
use worldwright_core::{
    ids::{DefinitionId, WorldId},
    rule::RuleRegistry,
};
fn example() -> Value {
    serde_json::from_str(include_str!("../../../examples/two-room/world.json")).unwrap()
}
fn instance(value: &Value) -> worldwright_content::WorldInstantiation {
    let content = load_bytes(
        "test",
        &serde_json::to_vec(value).unwrap(),
        Default::default(),
    )
    .unwrap();
    instantiate(&content, WorldId::from_u128(88).unwrap()).unwrap()
}
#[test]
fn mapping_is_independent_of_source_order() {
    let mut doc = example();
    let first = instance(&doc);
    doc["rooms"].as_array_mut().unwrap().reverse();
    let second = instance(&doc);
    assert_eq!(first.room_ids(), second.room_ids());
    assert_eq!(first.named_exits(), second.named_exits());
    assert_eq!(first.entry_room(), second.entry_room());
    let (_, a) = first.into_parts();
    let (_, b) = second.into_parts();
    assert_eq!(a, b);
}
#[test]
fn labels_and_entry_are_retained_and_builder_is_real() {
    let world = instance(&example());
    let atrium = world.room_ids()[&DefinitionId::parse("reference:rooms.atrium").unwrap()];
    let observatory =
        world.room_ids()[&DefinitionId::parse("reference:rooms.observatory").unwrap()];
    assert_eq!(world.entry_room(), atrium);
    assert_eq!(world.named_exits()[&atrium]["north"], observatory);
    assert_eq!(world.named_exits()[&observatory]["south"], atrium);
    let runtime = world.into_builder().build(RuleRegistry::new()).unwrap();
    assert_eq!(
        runtime.snapshot().world_id(),
        WorldId::from_u128(88).unwrap()
    );
    assert_eq!(runtime.snapshot().revision(), 0);
}
#[test]
fn adding_definitions_is_not_a_persistent_id_guarantee() {
    let mut doc = example();
    let before = instance(&doc);
    doc["rooms"].as_array_mut().unwrap().push(json!({"id":"reference:rooms.aaa", "title":"A", "description":"D", "exits":{}, "allow_unreachable":true}));
    let after = instance(&doc);
    assert_ne!(before.entry_room(), after.entry_room());
}
#[test]
fn isolated_and_forward_referenced_rooms_instantiate() {
    let mut doc = example();
    doc["rooms"][0]["exits"] = json!({});
    doc["rooms"][1]["allow_unreachable"] = json!(true);
    assert_eq!(instance(&doc).room_ids().len(), 2);
}
