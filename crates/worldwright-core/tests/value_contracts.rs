use worldwright_core::{
    command::{ConnectionGeneration, InputError, PlayerText},
    effect::{Audience, Effect, EffectPlan},
    ids::{CharacterId, DefinitionId, IdError, WorldId},
    time::{Tick, TimeError},
};

#[test]
fn ids_round_trip_and_reject_zero() {
    let id = WorldId::from_u128(0xab).unwrap();
    assert_eq!(id.to_string().parse::<WorldId>().unwrap(), id);
    assert_eq!(format!("{:032X}", id.get()).parse::<WorldId>().unwrap(), id);
    assert_eq!(WorldId::from_u128(0), Err(IdError::Zero));
    assert_eq!(
        "00000000000000000000000000000000".parse::<WorldId>(),
        Err(IdError::Zero)
    );
}

#[test]
fn ids_reject_unbounded_or_noncanonical_input() {
    for value in [
        "",
        "1",
        "+0000000000000000000000000000001",
        "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
    ] {
        assert_eq!(value.parse::<WorldId>(), Err(IdError::InvalidHex));
    }
    assert!("0".repeat(1024).parse::<WorldId>().is_err());
}

#[test]
fn definitions_validate_namespace_and_segments() {
    let id: DefinitionId = "frontier:rooms.arrival".parse().unwrap();
    assert_eq!(id.as_str(), "frontier:rooms.arrival");
    for value in [
        "room",
        "x:",
        ":x",
        "X:room",
        "x:rooms..entry",
        "x:../file",
        "x:y:z",
        "x:1room",
    ] {
        assert!(DefinitionId::parse(value).is_err(), "accepted {value}");
    }
    assert!(DefinitionId::parse(&format!("x:{}", "a".repeat(192))).is_err());
}

#[test]
fn text_preserves_unicode_but_is_not_html_escaping() {
    let value = "Hello, 星 — <em>untrusted markup</em>";
    assert_eq!(PlayerText::new(value).unwrap().as_str(), value);
}

#[test]
fn text_limits_are_bytes_not_character_counts() {
    assert!(PlayerText::new(&"a".repeat(PlayerText::MAX_BYTES)).is_ok());
    assert_eq!(
        PlayerText::new(&"a".repeat(PlayerText::MAX_BYTES + 1)),
        Err(InputError::TextTooLong)
    );
    assert_eq!(
        PlayerText::new(&"星".repeat(1400)),
        Err(InputError::TextTooLong)
    );
}

#[test]
fn text_rejects_empty_and_control_sequences() {
    for text in ["", "  "] {
        assert_eq!(PlayerText::new(text), Err(InputError::EmptyText));
    }
    for text in ["a\nb", "a\tb", "\u{1b}[31m", "x\u{85}"] {
        assert_eq!(PlayerText::new(text), Err(InputError::ControlCharacter));
    }
}

#[test]
fn generations_cannot_be_zero() {
    assert_eq!(
        ConnectionGeneration::new(0),
        Err(InputError::ZeroGeneration)
    );
    assert_eq!(ConnectionGeneration::new(7).unwrap().get(), 7);
}

#[test]
fn plans_are_bounded_without_partial_append_on_failure() {
    let mut plan = EffectPlan::new();
    let effect = Effect::EmitMessage {
        audience: Audience::Character(CharacterId::from_u128(1).unwrap()),
        text: PlayerText::new("hello").unwrap(),
    };
    for _ in 0..EffectPlan::MAX_EFFECTS {
        plan.push(effect.clone()).unwrap();
    }
    assert!(plan.push(effect).is_err());
    assert_eq!(plan.effects().len(), EffectPlan::MAX_EFFECTS);
    assert_eq!(plan.into_effects().len(), EffectPlan::MAX_EFFECTS);
}

#[test]
fn logical_time_checks_overflow() {
    assert_eq!(
        Tick::from_millis(10).checked_add(5).unwrap().as_millis(),
        15
    );
    assert_eq!(
        Tick::from_millis(u64::MAX).checked_add(1),
        Err(TimeError::Overflow)
    );
}
