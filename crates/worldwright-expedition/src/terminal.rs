//! Playable single-player adapter shared by generated games and artifact preview.
//! State is ephemeral. Scheduled recovery is applied on the next command after
//! its deadline in this blocking terminal adapter. The multiplayer owner also
//! advances scheduled work while idle; local terminal progress remains ephemeral.
use crate::{ExpeditionGame, action_id, initialize_character};
use std::{
    error::Error,
    io::{self, BufRead, IsTerminal, Write},
    process::ExitCode,
    time::Instant,
};
use worldwright_sdk::{
    GameModule,
    command::{ActorContext, CommandEnvelope, CommandIntent, PlayerText},
    content::{ValidatedWorldContent, WorldLookup, instantiate, load_bytes},
    ids::{AccountId, CharacterId, ConnectionId, DefinitionId, OperationId, WorldId},
    rule::{RuleError, RuleRegistry},
    runtime::{WorldError, WorldRuntime},
    time::{RandomExhausted, RandomSource, Tick},
};

pub const HELP: &str = "Commands: look (l), status, supplies, threat, victories, rank, go <exit>, say <text>, forage, encounter, strike, rest, help, quit (exit)";
const MAX_LINE_BYTES: usize = PlayerText::MAX_BYTES + 4;

struct SystemRandom;
impl RandomSource for SystemRandom {
    fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
        let mut bytes = [0; 8];
        getrandom::fill(&mut bytes).map_err(|_| RandomExhausted)?;
        Ok(u64::from_le_bytes(bytes))
    }
}

/// Run an executable's embedded, authoritative content without checkout paths.
pub fn run(bytes: &[u8]) -> ExitCode {
    match load_bytes("embedded expedition content", bytes, Default::default()) {
        Ok(content) => run_content(&content, None),
        Err(report) => {
            eprintln!("{report}");
            ExitCode::from(1)
        }
    }
}

/// Run validated artifact content; the caller verifies the artifact digest.
pub fn run_content(content: &ValidatedWorldContent, digest: Option<&str>) -> ExitCode {
    run_profile(content, digest, true)
}

/// Play the reference rules from relocatable embedded content.
pub fn run_reference(bytes: &[u8]) -> ExitCode {
    match load_bytes("embedded reference content", bytes, Default::default()) {
        Ok(content) => run_profile(&content, None, false),
        Err(report) => { eprintln!("{report}"); ExitCode::from(1) }
    }
}

fn run_profile(content: &ValidatedWorldContent, digest: Option<&str>, expedition: bool) -> ExitCode {
    let result = (|| -> Result<(), Box<dyn Error>> {
        let mut session = if expedition { Session::new(content)? } else { Session::new_profile(content, false)? };
        let stdin = io::stdin();
        let stdout = io::stdout();
        let prompt = stdin.is_terminal() && stdout.is_terminal();
        let mut input = stdin.lock();
        let mut output = stdout.lock();
        let result = (|| -> Result<(), Box<dyn Error>> {
            if let Some(digest) = digest {
                write_line(&mut output, &format!("Artifact digest: {digest}"))?;
                writeln!(
                    output,
                    "Worldwright Isolated Preview v0.1 — expedition game; no login, network, persistence, or production credentials."
                )?;
            } else {
                writeln!(
                    output,
                    "Worldwright {} — one local player; no login, network, or persistence.",
                    if expedition { "Expedition" } else { "Reference" }
                )?;
            }
            writeln!(output, "{}", if expedition { HELP } else { "Commands: look (l), go <exit>, say <text>, help, quit (exit)" })?;
            session.apply("look", Tick::ZERO, &mut SystemRandom, &mut output)?;
            let started = Instant::now();
            loop {
                if prompt {
                    write!(output, "> ")?;
                }
                output.flush()?;
                let Some(line) = read_line(&mut input)? else {
                    break;
                };
                let Some(line) = line else {
                    writeln!(
                        output,
                        "Input must be valid UTF-8 and at most {MAX_LINE_BYTES} bytes; entire line discarded."
                    )?;
                    continue;
                };
                let millis = u64::try_from(started.elapsed().as_millis())?;
                if session.apply(
                    &line,
                    Tick::from_millis(millis),
                    &mut SystemRandom,
                    &mut output,
                )? {
                    break;
                }
            }
            writeln!(output, "Goodbye.")?;
            output.flush()?;
            Ok(())
        })();
        // EOF, quit and I/O/runtime failures all attempt attachment cleanup.
        let detached = session.runtime.detach(session.actor);
        result?;
        detached?;
        Ok(())
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "worldwright expedition: {}",
                worldwright_sdk::content::terminal_safe(&error.to_string())
            );
            ExitCode::from(1)
        }
    }
}

struct Session {
    runtime: WorldRuntime,
    lookup: WorldLookup,
    actor: ActorContext,
    world: WorldId,
    next_operation: u128,
}
impl Session {
    fn new(content: &ValidatedWorldContent) -> Result<Self, Box<dyn Error>> {
        Self::new_profile(content, true)
    }
    fn new_profile(content: &ValidatedWorldContent, expedition: bool) -> Result<Self, Box<dyn Error>> {
        let world = WorldId::from_u128(1)?;
        let account = AccountId::from_u128(1)?;
        let character = CharacterId::from_u128(1)?;
        let (mut builder, lookup) = instantiate(content, world)?.into_parts();
        builder.add_character(character, account, lookup.entry_room)?;
        let mut rules = RuleRegistry::new();
        if expedition {
            initialize_character(&mut builder, character)?;
            ExpeditionGame.register_rules(&mut rules)?;
        } else {
            worldwright_sdk::runtime::ReferenceGame.register_rules(&mut rules)?;
        }
        let mut runtime = builder.build(rules)?;
        let actor = runtime.attach(account, character, ConnectionId::from_u128(1)?)?;
        Ok(Self {
            runtime,
            lookup,
            actor,
            world,
            next_operation: 1,
        })
    }
    fn execute(
        &mut self,
        intent: CommandIntent,
        now: Tick,
        random: &mut impl RandomSource,
        output: &mut impl Write,
    ) -> Result<(), Box<dyn Error>> {
        let operation = OperationId::from_u128(self.next_operation)?;
        self.next_operation = self
            .next_operation
            .checked_add(1)
            .ok_or(WorldError::CounterExhausted)?;
        let result = self.runtime.execute(
            &CommandEnvelope {
                operation,
                world: self.world,
                actor: self.actor,
                intent,
            },
            now,
            random,
        )?;
        for message in result.messages {
            if message.recipient == self.actor
                && self.runtime.snapshot().attachment(self.actor.character) == Some(self.actor)
            {
                write_line(output, message.text.as_str())?;
            }
        }
        Ok(())
    }
    fn look(
        &mut self,
        now: Tick,
        random: &mut impl RandomSource,
        output: &mut impl Write,
    ) -> Result<(), Box<dyn Error>> {
        self.execute(CommandIntent::Look, now, random, output)?;
        let room = self
            .runtime
            .snapshot()
            .room_of(self.actor.character)
            .ok_or(WorldError::UnknownRoom)?;
        let exits = self
            .lookup
            .named_exits
            .get(&room)
            .ok_or(WorldError::UnknownRoom)?;
        writeln!(
            output,
            "Exits: {}",
            if exits.is_empty() {
                "(none)".into()
            } else {
                exits.keys().cloned().collect::<Vec<_>>().join(", ")
            }
        )?;
        Ok(())
    }
    fn apply(
        &mut self,
        line: &str,
        now: Tick,
        random: &mut impl RandomSource,
        output: &mut impl Write,
    ) -> Result<bool, Box<dyn Error>> {
        if line.chars().any(|c| c.is_control() && c != '\t') {
            writeln!(output, "Control characters are not allowed.")?;
            return Ok(false);
        }
        let line = line.trim_start_matches([' ', '\t']);
        let split = line.find([' ', '\t']).unwrap_or(line.len());
        let verb = &line[..split];
        let payload = line[split..].trim_start_matches([' ', '\t']);
        if verb.is_empty() {
            return Ok(false);
        }
        if matches!(verb, "quit" | "exit") && payload.is_empty() {
            return Ok(true);
        }
        if verb == "help" && payload.is_empty() {
            writeln!(output, "{HELP}")?;
            return Ok(false);
        }
        let mut refresh = false;
        let intent = if matches!(
            verb,
            "look" | "l" | "status" | "supplies" | "threat" | "victories" | "rank"
        ) && payload.is_empty()
        {
            CommandIntent::Look
        } else if let Some(action) = action_id(verb).filter(|_| payload.is_empty()) {
            refresh = true;
            CommandIntent::GameAction {
                action: DefinitionId::parse(action)?,
                argument: None,
            }
        } else if verb == "go" && !payload.is_empty() && !payload.contains(char::is_whitespace) {
            let room = self
                .runtime
                .snapshot()
                .room_of(self.actor.character)
                .ok_or(WorldError::UnknownRoom)?;
            let Some(destination) = self
                .lookup
                .named_exits
                .get(&room)
                .and_then(|exits| exits.get(payload))
                .copied()
            else {
                writeln!(
                    output,
                    "No such exit from this room. Use look to list exits."
                )?;
                return Ok(false);
            };
            refresh = true;
            CommandIntent::Move { destination }
        } else if verb == "say" {
            match PlayerText::new(payload) {
                Ok(text) => CommandIntent::Say { text },
                Err(_) => {
                    writeln!(
                        output,
                        "Usage: say <text> (nonblank, at most 4096 UTF-8 bytes, no controls)"
                    )?;
                    return Ok(false);
                }
            }
        } else {
            writeln!(output, "Unknown command or invalid arguments. {HELP}")?;
            return Ok(false);
        };
        self.runtime.advance(now)?;
        let is_look = matches!(intent, CommandIntent::Look);
        let result = if is_look {
            self.look(now, random, output)
        } else {
            self.execute(intent, now, random, output)
        };
        match result {
            Ok(()) => {
                if refresh {
                    self.look(now, random, output)?;
                }
            }
            Err(error)
                if matches!(
                    error.downcast_ref::<WorldError>(),
                    Some(WorldError::Rule(RuleError::Rejected))
                ) =>
            {
                writeln!(
                    output,
                    "Action rejected by expedition rules; the command made no changes. Due scheduled recovery may still have applied. Use status to inspect supplies and threat."
                )?;
            }
            Err(error) => return Err(error),
        }
        Ok(false)
    }
}

// None = EOF; Some(None) = rejected complete line. Drain oversized input so its
// suffix cannot be interpreted as a separate command. Allocation stays bounded.
fn read_line(reader: &mut impl BufRead) -> io::Result<Option<Option<String>>> {
    let mut bytes = Vec::new();
    let mut overflow = false;
    let mut seen = false;
    loop {
        let chunk = match reader.fill_buf() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            value => value?,
        };
        if chunk.is_empty() {
            if !seen {
                return Ok(None);
            }
            break;
        }
        seen = true;
        let newline = chunk.iter().position(|b| *b == b'\n');
        let length = newline.unwrap_or(chunk.len());
        let copied = length.min((MAX_LINE_BYTES + 1).saturating_sub(bytes.len()));
        bytes.extend_from_slice(&chunk[..copied]);
        overflow |= copied != length;
        reader.consume(length + usize::from(newline.is_some()));
        if newline.is_some() {
            break;
        }
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    Ok(Some(if overflow || bytes.len() > MAX_LINE_BYTES {
        None
    } else {
        String::from_utf8(bytes).ok()
    }))
}
fn write_line(output: &mut impl Write, text: &str) -> io::Result<()> {
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
mod tests {
    use super::*;
    struct FixedRandom;
    impl RandomSource for FixedRandom {
        fn next_u64(&mut self) -> Result<u64, RandomExhausted> {
            Ok(0)
        }
    }
    fn fixture() -> Session {
        let content = load_bytes(
            "test",
            include_bytes!("../../../examples/expedition/world.json"),
            Default::default(),
        )
        .unwrap();
        Session::new(&content).unwrap()
    }
    #[test]
    fn playable_host_runs_encounter_strikes_and_recovery() {
        let mut session = fixture();
        let mut output = Vec::new();
        for (at, command) in [
            (0, "look"),
            (1, "encounter"),
            (2, "strike"),
            (3, "strike"),
            (1003, "status"),
        ] {
            assert!(
                !session
                    .apply(
                        command,
                        Tick::from_millis(at),
                        &mut FixedRandom,
                        &mut output
                    )
                    .unwrap()
            );
        }
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("An encounter begins with threat 2."));
        assert!(text.contains("Victories: 1 | Rank: Scout"));
        assert!(text.contains("Supplies: 2 | Threat: 0 | Victories: 1"));
    }
    #[test]
    fn playable_host_rejection_rest_forage_and_quit() {
        let mut session = fixture();
        let mut output = Vec::new();
        for (at, command) in [
            (0, "strike"),
            (1, "rest"),
            (1001, "supplies"),
            (1002, "forage"),
            (1003, "go east"),
        ] {
            session
                .apply(
                    command,
                    Tick::from_millis(at),
                    &mut FixedRandom,
                    &mut output,
                )
                .unwrap();
        }
        assert!(
            session
                .apply(
                    "quit",
                    Tick::from_millis(1004),
                    &mut FixedRandom,
                    &mut output
                )
                .unwrap()
        );
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Action rejected by expedition rules"));
        assert!(text.contains("Supplies: 4 | Threat: 0"));
        assert!(text.contains("Foraging recovers 1 supplies."));
        assert!(text.contains("Shale Pass"));
    }
    #[test]
    fn executable_reaches_pathfinder_and_trailblazer() {
        let mut session = fixture();
        let mut output = Vec::new();
        let mut at = 0;
        for _ in 0..6 {
            while session
                .runtime
                .snapshot()
                .counter(
                    session.actor.character,
                    DefinitionId::parse(crate::SUPPLIES_KEY).unwrap(),
                )
                .unwrap()
                < crate::SUPPLY_CAPACITY
            {
                at += 1;
                session
                    .apply(
                        "forage",
                        Tick::from_millis(at),
                        &mut FixedRandom,
                        &mut output,
                    )
                    .unwrap();
            }
            for command in ["encounter", "strike", "strike"] {
                at += 1;
                session
                    .apply(
                        command,
                        Tick::from_millis(at),
                        &mut FixedRandom,
                        &mut output,
                    )
                    .unwrap();
            }
            at += 1_000;
            session
                .apply("rank", Tick::from_millis(at), &mut FixedRandom, &mut output)
                .unwrap();
        }
        let text = String::from_utf8(output).unwrap();
        assert!(text.contains("Victories: 3 | Rank: Pathfinder"));
        assert!(text.contains("Victories: 6 | Rank: Trailblazer"));
    }
    #[test]
    fn bounded_input_drains_invalid_lines() {
        let input = format!("{}quit\nlook\r\n", "x".repeat(MAX_LINE_BYTES + 1));
        let mut reader = io::Cursor::new(input);
        assert_eq!(read_line(&mut reader).unwrap(), Some(None));
        assert_eq!(read_line(&mut reader).unwrap(), Some(Some("look".into())));
        assert_eq!(read_line(&mut reader).unwrap(), None);
        assert_eq!(
            read_line(&mut io::Cursor::new(b"say \xff\n")).unwrap(),
            Some(None)
        );
    }
}
