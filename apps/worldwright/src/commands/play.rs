//! Trusted, single-player terminal host. Gameplay always passes through execute.
use super::play_input::{self, Input, Line, MAX_COMMAND_BYTES};
use std::{
    error::Error,
    fmt,
    io::{self, BufRead, IsTerminal, Write},
    path::Path,
    process::ExitCode,
    time::Instant,
};
use worldwright_content::{
    ValidatedWorldContent, WorldLookup, instantiate, load_file, terminal_safe,
};
use worldwright_sdk::{
    GameModule,
    command::{ActorContext, CommandEnvelope, CommandIntent},
    ids::{AccountId, CharacterId, ConnectionId, OperationId, WorldId},
    rule::RuleRegistry,
    runtime::{ExecutionResult, ReferenceGame, WorldError, WorldRuntime},
    time::{RandomExhausted, RandomSource, Tick},
};

const HELP: &str = "Commands: look (l), go <exit-label>, say <text>, help, quit (exit)";

pub(super) fn play(path: &Path) -> ExitCode {
    let content = match load_file(path, Default::default()) {
        Ok(content) => content,
        Err(report) => {
            let _ = writeln!(io::stderr().lock(), "{report}");
            return ExitCode::from(1);
        }
    };
    let mut session = match Session::new(&content) {
        Ok(session) => session,
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "worldwright play: startup failed: {}",
                terminal_safe(&error.to_string())
            );
            return ExitCode::from(1);
        }
    };
    let stdin = io::stdin();
    let stdout = io::stdout();
    let prompt = stdin.is_terminal() && stdout.is_terminal();
    match run_session(
        &mut session,
        &mut stdin.lock(),
        &mut stdout.lock(),
        &mut io::stderr().lock(),
        prompt,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(
                io::stderr().lock(),
                "worldwright play: {}",
                terminal_safe(&error.to_string())
            );
            ExitCode::from(1)
        }
    }
}

#[derive(Debug)]
enum PlayError {
    Runtime(WorldError),
    Io(io::Error),
    State(&'static str),
}
impl fmt::Display for PlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Runtime(WorldError::Unreachable | WorldError::UnknownRoom) => {
                f.write_str("Movement rejected: destination is not reachable.")
            }
            Self::Runtime(WorldError::CounterExhausted) => {
                f.write_str("Runtime counter exhausted; session stopped.")
            }
            Self::Runtime(
                WorldError::NotAttached | WorldError::StaleAttachment | WorldError::OwnerMismatch,
            ) => f.write_str("Local player attachment is no longer valid."),
            Self::Runtime(_) => f.write_str("The runtime rejected the action; session stopped."),
            Self::Io(error) => write!(f, "terminal I/O failed: {error}"),
            Self::State(message) => f.write_str(message),
        }
    }
}
impl Error for PlayError {}
impl From<WorldError> for PlayError {
    fn from(e: WorldError) -> Self {
        Self::Runtime(e)
    }
}
impl From<io::Error> for PlayError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

// ReferenceGame never asks for randomness. Do not silently supply fake random
// values if a future game changes that contract; a real adapter is then needed.
struct NoRandom;
impl RandomSource for NoRandom {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
        Err(RandomExhausted)
    }
}

struct Session {
    runtime: WorldRuntime,
    lookup: WorldLookup,
    world: WorldId,
    actor: ActorContext,
    next_operation: u128,
    started: Instant,
}
impl Session {
    fn new(content: &ValidatedWorldContent) -> Result<Self, Box<dyn Error>> {
        // Ephemeral host-owned identities; no login, persisted ID or session claim.
        let world = WorldId::from_u128(1)?;
        let account = AccountId::from_u128(1)?;
        let character = CharacterId::from_u128(1)?;
        let connection = ConnectionId::from_u128(1)?;
        let (mut builder, lookup) = instantiate(content, world)?.into_parts();
        builder.add_character(character, account, lookup.entry_room)?;
        let mut rules = RuleRegistry::new();
        ReferenceGame.register_rules(&mut rules)?;
        let mut runtime = builder.build(rules)?;
        let actor = runtime.attach(account, character, connection)?;
        Ok(Self {
            runtime,
            lookup,
            world,
            actor,
            next_operation: 1,
            started: Instant::now(),
        })
    }
    fn current_room(&self) -> Result<worldwright_sdk::ids::RoomId, PlayError> {
        // Snapshot is host-only. Project ONLY this actor's room, never dump it.
        let snapshot = self.runtime.snapshot();
        if snapshot.attachment(self.actor.character) != Some(self.actor) {
            return Err(PlayError::State(
                "Local player attachment is no longer valid.",
            ));
        }
        snapshot
            .room_of(self.actor.character)
            .ok_or(PlayError::State("Local player has no room."))
    }
    fn execute(&mut self, intent: CommandIntent) -> Result<ExecutionResult, PlayError> {
        let operation = OperationId::from_u128(self.next_operation)
            .map_err(|_| PlayError::State("Operation identity exhausted; session stopped."))?;
        let next = self.next_operation.checked_add(1).ok_or(PlayError::State(
            "Operation identity exhausted; session stopped.",
        ))?;
        let millis = u64::try_from(self.started.elapsed().as_millis())
            .map_err(|_| PlayError::State("Monotonic time exhausted; session stopped."))?;
        self.next_operation = next; // IDs are consumed even when an action is rejected.
        Ok(self.runtime.execute(
            &CommandEnvelope {
                operation,
                world: self.world,
                actor: self.actor,
                intent,
            },
            Tick::from_millis(millis),
            &mut NoRandom,
        )?)
    }
    fn deliver(&self, result: &ExecutionResult, output: &mut impl Write) -> Result<(), PlayError> {
        if result.world != self.world {
            return Err(PlayError::State("Unexpected result world."));
        }
        let attachment = self.runtime.snapshot().attachment(self.actor.character);
        for message in &result.messages {
            if message.recipient == self.actor && attachment == Some(self.actor) {
                write_player_line(output, message.text.as_str())?;
            }
        }
        Ok(())
    }
    fn look(&mut self, output: &mut impl Write) -> Result<(), PlayError> {
        let result = self.execute(CommandIntent::Look)?;
        self.deliver(&result, output)?;
        let room = self.current_room()?;
        let exits = self
            .lookup
            .named_exits
            .get(&room)
            .ok_or(PlayError::State("Current-room exit metadata is missing."))?;
        if exits.is_empty() {
            writeln!(output, "Exits: (none)")?;
        } else {
            // Labels are schema-validated ASCII. Do not expose other rooms/IDs.
            writeln!(
                output,
                "Exits: {}",
                exits
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            )?;
        }
        Ok(())
    }
    fn action(
        &mut self,
        input: Input<'_>,
        output: &mut impl Write,
        errors: &mut impl Write,
    ) -> Result<bool, PlayError> {
        match input {
            Input::Blank => {}
            Input::Help => writeln!(output, "{HELP}")?,
            Input::Quit => return Ok(true),
            Input::Look => self.look(output)?,
            Input::Say(text) => {
                let result = self.execute(CommandIntent::Say { text })?;
                self.deliver(&result, output)?;
            }
            Input::Go(label) => {
                let room = self.current_room()?;
                let destination = self
                    .lookup
                    .named_exits
                    .get(&room)
                    .and_then(|exits| exits.get(label))
                    .copied();
                let Some(destination) = destination else {
                    writeln!(
                        errors,
                        "No such exit from this room. Use look to list exits."
                    )?;
                    return Ok(false);
                };
                // Lookup resolution is not authorization: runtime checks the edge.
                let result = self.execute(CommandIntent::Move { destination })?;
                self.deliver(&result, output)?;
                self.look(output)?; // Separate runtime action, not part of Move's transaction.
            }
        }
        Ok(false)
    }
}

fn run_session(
    session: &mut Session,
    input: &mut impl BufRead,
    output: &mut impl Write,
    errors: &mut impl Write,
    prompt: bool,
) -> Result<(), PlayError> {
    let result = (|| {
        writeln!(
            output,
            "Worldwright Local Play v0.1 — one local player; no login, network, or persistence."
        )?;
        writeln!(output, "{HELP}")?;
        session.look(output)?;
        loop {
            if prompt {
                write!(output, "> ")?;
            }
            output.flush()?;
            match play_input::read_line(input)? {
                Line::Eof => break,
                Line::TooLong => writeln!(
                    errors,
                    "Input exceeds {MAX_COMMAND_BYTES} bytes; entire line discarded."
                )?,
                Line::InvalidUtf8 => {
                    writeln!(errors, "Input must be valid UTF-8; line discarded.")?
                }
                Line::Text(line) => match play_input::parse(&line) {
                    Ok(command) => {
                        if session.action(command, output, errors)? {
                            break;
                        }
                    }
                    Err(message) => writeln!(errors, "{message}")?,
                },
            }
            errors.flush()?;
        }
        writeln!(output, "Goodbye.")?;
        output.flush()?;
        Ok(())
    })();
    // Attempt cleanup on EOF, quit AND I/O/runtime failure. Preserve the original
    // error if both loop and detach fail. The world is dropped by the host next.
    let detached = session
        .runtime
        .detach(session.actor)
        .map_err(PlayError::Runtime);
    result.and(detached)
}

fn write_player_line(output: &mut impl Write, text: &str) -> io::Result<()> {
    // Unlike short diagnostic labels, valid 4096-byte descriptions are not
    // truncated. Keep ordinary Unicode, escape controls and bidi/line controls.
    for ch in text.chars() {
        if ch.is_control()
            || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        {
            write!(output, "{}", ch.escape_default())?;
        } else {
            write!(output, "{ch}")?;
        }
    }
    writeln!(output)
}

#[cfg(test)]
mod tests;
