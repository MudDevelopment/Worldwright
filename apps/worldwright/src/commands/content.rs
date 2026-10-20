use std::{path::Path, process::ExitCode};
use worldwright_content::{instantiate, load_file, terminal_safe};
use worldwright_sdk::{ids::WorldId, rule::RuleRegistry};

pub fn validate(path: &Path) -> ExitCode {
    let content = match load_file(path, Default::default()) {
        Ok(content) => content,
        Err(report) => {
            eprint!("{report}");
            return ExitCode::from(1);
        }
    };
    // A throwaway identity proves room/edge construction only. It is not a
    // durable world ID, account identity, running game, or activation request.
    let world = match WorldId::from_u128(1) {
        Ok(world) => world,
        Err(error) => {
            eprintln!("worldwright: {error}");
            return ExitCode::from(1);
        }
    };
    let instance = match instantiate(&content, world) {
        Ok(instance) => instance,
        Err(report) => {
            eprint!("{report}");
            return ExitCode::from(1);
        }
    };
    if let Err(error) = instance.into_builder().build(RuleRegistry::new()) {
        eprintln!("error[WWC_BUILD]: {error}");
        return ExitCode::from(1);
    }
    println!("Valid content: {}", terminal_safe(&path.to_string_lossy()));
    println!("Schema: {}", content.schema_version());
    println!("Namespace: {}", content.namespace());
    println!("Rooms: {}", content.rooms().len());
    println!("Directed exits: {}", content.exit_count());
    println!("Entry room: {}", content.entry_room());
    ExitCode::SUCCESS
}
