use super::*;
use std::io::Cursor;
use worldwright_content::load_bytes;
use worldwright_sdk::command::{ConnectionGeneration, PlayerText};
const EXAMPLE: &[u8] = include_bytes!("../../../../../examples/two-room/world.json");
fn session() -> Session {
    Session::new(&load_bytes("test", EXAMPLE, Default::default()).unwrap()).unwrap()
}

#[test]
fn actions_use_runtime_and_time_never_regresses() {
    let mut s = session();
    let original_room = s.current_room().unwrap();
    let revision = s.runtime.snapshot().revision();
    let look = s.execute(CommandIntent::Look).unwrap();
    assert_eq!(look.revision, revision + 1);
    let first_time = s.runtime.snapshot().last_execution().unwrap();
    assert_eq!(look.messages[0].text.as_str(), "Atrium");
    let mut output = Vec::new();
    s.action(Input::Go("north"), &mut output, &mut Vec::new())
        .unwrap();
    assert_ne!(s.current_room().unwrap(), original_room);
    assert_eq!(s.runtime.snapshot().revision(), revision + 3); // Look, Move, auto-Look
    assert!(s.runtime.snapshot().last_execution().unwrap() >= first_time);
    let say = s
        .execute(CommandIntent::Say {
            text: PlayerText::new("hello").unwrap(),
        })
        .unwrap();
    assert_eq!(say.operation, OperationId::from_u128(4).unwrap());
    assert_eq!(say.messages.len(), 1);
    assert_eq!(say.messages[0].recipient, s.actor);
}
#[test]
fn unknown_exit_help_and_blank_do_not_mutate_world() {
    let mut s = session();
    let before = s.runtime.snapshot();
    let mut errors = Vec::new();
    for input in [Input::Blank, Input::Help, Input::Go("missing")] {
        s.action(input, &mut Vec::new(), &mut errors).unwrap();
        assert_eq!(s.runtime.snapshot(), before);
    }
    assert_eq!(s.next_operation, 1);
    assert!(String::from_utf8(errors).unwrap().contains("No such exit"));
}
#[test]
fn corrupt_lookup_cannot_bypass_directed_runtime_edges() {
    let source = std::str::from_utf8(EXAMPLE)
        .unwrap()
        .replace("\"south\": \"reference:rooms.atrium\"", "");
    let mut s =
        Session::new(&load_bytes("one-way", source.as_bytes(), Default::default()).unwrap())
            .unwrap();
    let atrium = s.current_room().unwrap();
    s.action(Input::Go("north"), &mut Vec::new(), &mut Vec::new())
        .unwrap();
    let observatory = s.current_room().unwrap();
    // Deliberately falsify host metadata; engine must still refuse the edge.
    s.lookup
        .named_exits
        .get_mut(&observatory)
        .unwrap()
        .insert("south".into(), atrium);
    let before = s.runtime.snapshot();
    let mut output = Vec::new();
    assert!(matches!(
        s.action(Input::Go("south"), &mut output, &mut Vec::new()),
        Err(PlayError::Runtime(WorldError::Unreachable))
    ));
    assert_eq!(s.runtime.snapshot(), before);
    assert!(output.is_empty());
}
#[test]
fn only_current_exact_attachment_receives_messages() {
    let mut s = session();
    let mut result = s
        .execute(CommandIntent::Say {
            text: PlayerText::new("visible").unwrap(),
        })
        .unwrap();
    let mut stale = result.messages[0].clone();
    stale.recipient.generation = ConnectionGeneration::new(s.actor.generation.get() + 1).unwrap();
    stale.text = PlayerText::new("hidden").unwrap();
    result.messages.push(stale);
    let mut out = Vec::new();
    s.deliver(&result, &mut out).unwrap();
    assert_eq!(out, b"visible\n");
    s.runtime.detach(s.actor).unwrap();
    out.clear();
    s.deliver(&result, &mut out).unwrap();
    assert!(out.is_empty());
}
#[test]
fn counter_exhaustion_does_not_wrap_or_execute() {
    let mut s = session();
    s.next_operation = u128::MAX;
    let before = s.runtime.snapshot();
    assert!(matches!(
        s.execute(CommandIntent::Look),
        Err(PlayError::State(_))
    ));
    assert_eq!(s.runtime.snapshot(), before);
    assert_eq!(s.next_operation, u128::MAX);
}
#[test]
fn quit_eof_and_final_line_detach_without_piped_prompts() {
    for input in [b"".as_slice(), b"quit\n", b"exit", b"say final"] {
        let mut s = session();
        let mut out = Vec::new();
        run_session(
            &mut s,
            &mut Cursor::new(input),
            &mut out,
            &mut Vec::new(),
            false,
        )
        .unwrap();
        assert!(s.runtime.snapshot().attachment(s.actor.character).is_none());
        let text = String::from_utf8(out).unwrap();
        assert!(text.ends_with("Goodbye.\n"));
        assert!(!text.contains("> "));
    }
}
#[test]
fn terminal_prompt_mode_is_explicit() {
    let mut s = session();
    let mut out = Vec::new();
    run_session(
        &mut s,
        &mut Cursor::new(b"quit\n"),
        &mut out,
        &mut Vec::new(),
        true,
    )
    .unwrap();
    assert!(String::from_utf8(out).unwrap().contains("> Goodbye."));
}
struct ReadFailure;
impl io::Read for ReadFailure {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("read test"))
    }
}
impl BufRead for ReadFailure {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Err(io::Error::other("read test"))
    }
    fn consume(&mut self, _: usize) {}
}
struct WriteFailure;
impl Write for WriteFailure {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "write test"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn input_and_output_errors_still_detach() {
    let mut s = session();
    assert!(matches!(
        run_session(
            &mut s,
            &mut ReadFailure,
            &mut Vec::new(),
            &mut Vec::new(),
            false
        ),
        Err(PlayError::Io(_))
    ));
    assert!(s.runtime.snapshot().attachment(s.actor.character).is_none());
    let mut s = session();
    assert!(matches!(
        run_session(
            &mut s,
            &mut Cursor::new(b""),
            &mut WriteFailure,
            &mut Vec::new(),
            false
        ),
        Err(PlayError::Io(_))
    ));
    assert!(s.runtime.snapshot().attachment(s.actor.character).is_none());
}
#[test]
fn rendering_preserves_full_text_unicode_and_escapes_controls() {
    let text = "é".repeat(2048);
    let mut output = Vec::new();
    write_player_line(&mut output, &text).unwrap();
    assert_eq!(output.len(), 4097);
    assert_eq!(String::from_utf8(output).unwrap(), format!("{text}\n"));
    let mut output = Vec::new();
    write_player_line(&mut output, "café\u{202e}\x1b").unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(output.contains("café"));
    assert!(!output.contains('\u{202e}'));
    assert!(!output.contains('\x1b'));
}
