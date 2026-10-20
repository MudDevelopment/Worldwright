use worldwright_sdk::{
    GameModule, ModuleRegistrationError,
    command::{CommandIntent, CommandKind},
    effect::EffectPlan,
    rule::{Rule, RuleContext, RuleError, RuleRegistry},
    time::RandomSource,
};

struct NoEffects;
impl Rule for NoEffects {
    fn evaluate(
        &self,
        _: &RuleContext<'_>,
        _: &CommandIntent,
        _: &mut dyn RandomSource,
    ) -> Result<EffectPlan, RuleError> {
        Ok(EffectPlan::new())
    }
}
struct ExampleModule;
impl GameModule for ExampleModule {
    fn name(&self) -> &str {
        "registration test"
    }
    fn register_rules(&self, registry: &mut RuleRegistry) -> Result<(), ModuleRegistrationError> {
        registry.register(CommandKind::Look, NoEffects)?;
        Ok(())
    }
}

#[test]
fn game_registers_rules_and_reports_duplicate_errors() {
    let mut registry = RuleRegistry::new();
    ExampleModule.register_rules(&mut registry).unwrap();
    assert!(registry.get(CommandKind::Look).is_some());
    assert!(matches!(
        ExampleModule.register_rules(&mut registry),
        Err(ModuleRegistrationError::Duplicate(_))
    ));
}
