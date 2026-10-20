//! Bounded line framing for interactive and piped input. No unbounded read_line.
use std::io::{self, BufRead};
use worldwright_sdk::command::PlayerText;

// "say " plus a maximum-size PlayerText; one extra byte is reserved for CRLF.
pub(super) const MAX_COMMAND_BYTES: usize = PlayerText::MAX_BYTES + 4;
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Line {
    Eof,
    Text(String),
    TooLong,
    InvalidUtf8,
}

pub(super) fn read_line(reader: &mut impl BufRead) -> io::Result<Line> {
    let mut bytes = Vec::new();
    let mut overflow = false;
    let mut saw_input = false;
    loop {
        let chunk = match reader.fill_buf() {
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if chunk.is_empty() {
            if !saw_input {
                return Ok(Line::Eof);
            }
            break;
        }
        saw_input = true;
        let newline = chunk.iter().position(|b| *b == b'\n');
        let length = newline.unwrap_or(chunk.len());
        let remaining = (MAX_COMMAND_BYTES + 1) - bytes.len();
        let copy = length.min(remaining);
        bytes.extend_from_slice(&chunk[..copy]);
        overflow |= length > copy;
        reader.consume(length + usize::from(newline.is_some()));
        if newline.is_some() {
            break;
        }
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if overflow || bytes.len() > MAX_COMMAND_BYTES {
        return Ok(Line::TooLong);
    }
    Ok(match String::from_utf8(bytes) {
        Ok(line) => Line::Text(line),
        Err(_) => Line::InvalidUtf8,
    })
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Input<'a> {
    Blank,
    Look,
    Go(&'a str),
    Say(PlayerText),
    Help,
    Quit,
}
pub(super) fn parse(line: &str) -> Result<Input<'_>, &'static str> {
    // Tabs may separate tokens but are never accepted inside PlayerText.
    if line.chars().any(|c| c.is_control() && c != '\t') {
        return Err("Control characters are not allowed.");
    }
    let line = line.trim_start_matches([' ', '\t']);
    if line.is_empty() {
        return Ok(Input::Blank);
    }
    let split = line.find([' ', '\t']).unwrap_or(line.len());
    let verb = &line[..split];
    let payload = line[split..].trim_start_matches([' ', '\t']);
    match verb {
        "look" | "l" if payload.is_empty() => Ok(Input::Look),
        "help" if payload.is_empty() => Ok(Input::Help),
        "quit" | "exit" if payload.is_empty() => Ok(Input::Quit),
        "go" => {
            let label = payload.trim_end_matches([' ', '\t']);
            if label.is_empty() || label.contains(char::is_whitespace) {
                Err("Usage: go <exit-label>")
            } else {
                Ok(Input::Go(label))
            }
        }
        "say" => PlayerText::new(payload).map(Input::Say).map_err(
            |_| "Usage: say <text> (1..4096 UTF-8 bytes, nonblank, no control characters)",
        ),
        "look" | "l" | "help" | "quit" | "exit" => Err("This command takes no arguments."),
        _ => Err("Unknown command. Type help for available commands."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};
    #[test]
    fn framing_limits_crlf_and_unterminated_eof() {
        for suffix in ["\n", "\r\n", "", "\r"] {
            let input = format!("{}{}", "x".repeat(MAX_COMMAND_BYTES), suffix);
            let mut reader = BufReader::with_capacity(3, Cursor::new(input));
            assert!(
                matches!(read_line(&mut reader).unwrap(), Line::Text(s) if s.len() == MAX_COMMAND_BYTES)
            );
            assert_eq!(read_line(&mut reader).unwrap(), Line::Eof);
        }
    }
    #[test]
    fn oversized_line_drains_without_executing_suffix() {
        let input = format!("{}quit\nlook\n", "x".repeat(MAX_COMMAND_BYTES + 10));
        let mut reader = BufReader::with_capacity(7, Cursor::new(input));
        assert_eq!(read_line(&mut reader).unwrap(), Line::TooLong);
        assert_eq!(read_line(&mut reader).unwrap(), Line::Text("look".into()));
        assert_eq!(read_line(&mut reader).unwrap(), Line::Eof);
    }
    #[test]
    fn invalid_utf8_is_rejected_without_losing_next_line() {
        let mut reader = Cursor::new(b"say \xff\nlook");
        assert_eq!(read_line(&mut reader).unwrap(), Line::InvalidUtf8);
        assert_eq!(read_line(&mut reader).unwrap(), Line::Text("look".into()));
    }
    #[test]
    fn parsing_aliases_whitespace_and_payload() {
        assert_eq!(parse(" \t ").unwrap(), Input::Blank);
        assert_eq!(parse(" l\t").unwrap(), Input::Look);
        assert_eq!(parse(" go\tnorth  ").unwrap(), Input::Go("north"));
        assert_eq!(parse("exit").unwrap(), Input::Quit);
        assert_eq!(
            parse("say  hello café  ").unwrap(),
            Input::Say(PlayerText::new("hello café  ").unwrap())
        );
        for invalid in [
            "north",
            "LOOK",
            "go",
            "go north south",
            "look extra",
            "help x",
            "quit x",
            "say",
            "say a\tb",
            "say a\x1bb",
            "look\rgo north",
        ] {
            assert!(parse(invalid).is_err(), "{invalid:?}");
        }
    }
}
