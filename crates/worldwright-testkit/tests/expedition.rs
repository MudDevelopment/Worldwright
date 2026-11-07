use worldwright_expedition::{
    ENCOUNTER_ACTION, STRIKE_ACTION, SUPPLIES_KEY, THREAT_KEY, VICTORIES_KEY,
};
use worldwright_sdk::{command::CommandIntent, ids::DefinitionId, time::Tick};
use worldwright_testkit::{RuntimeScenario, RuntimeStep, SequenceRandom, expedition_world};

#[test]
fn independent_expedition_runs_content_rules_scenario_and_recovery() {
    let mut world = expedition_world().unwrap();
    let mut scenario = RuntimeScenario::new();
    for (at, action) in [
        (1, ENCOUNTER_ACTION),
        (2, STRIKE_ACTION),
        (3, STRIKE_ACTION),
    ] {
        scenario
            .push(RuntimeStep {
                actor: world.alice,
                intent: CommandIntent::GameAction {
                    action: DefinitionId::parse(action).unwrap(),
                    argument: None,
                },
                at: Tick::from_millis(at),
            })
            .unwrap();
    }
    let report = scenario
        .run(&mut world.runtime, &mut SequenceRandom::new([0]))
        .unwrap();
    assert_eq!(report.executions().len(), 3);
    let before = world.runtime.snapshot();
    assert_eq!(
        before.counter(
            world.alice.character,
            DefinitionId::parse(THREAT_KEY).unwrap()
        ),
        Some(0)
    );
    assert_eq!(
        before.counter(
            world.alice.character,
            DefinitionId::parse(VICTORIES_KEY).unwrap()
        ),
        Some(1)
    );
    assert_eq!(before.scheduled_actions(), 2);
    let due = world.runtime.advance(Tick::from_millis(1_003)).unwrap();
    assert_eq!(due.events.len(), 2);
    assert_eq!(
        world.runtime.snapshot().counter(
            world.alice.character,
            DefinitionId::parse(SUPPLIES_KEY).unwrap()
        ),
        Some(2)
    );
}
