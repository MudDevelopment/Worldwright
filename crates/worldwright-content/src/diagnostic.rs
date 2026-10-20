//! Structured bounded diagnostics. Rendering never emits raw source text.
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub location: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentReport {
    source: String,
    diagnostics: Vec<Diagnostic>,
    limit: usize,
    truncated: bool,
}
impl ContentReport {
    pub(crate) fn new(source: &str, limit: usize) -> Self {
        Self {
            source: source.into(),
            diagnostics: Vec::new(),
            limit: limit.clamp(1, 100),
            truncated: false,
        }
    }
    pub(crate) fn push(
        &mut self,
        code: &'static str,
        location: impl Into<String>,
        message: impl Into<String>,
    ) {
        if self.diagnostics.len() == self.limit {
            self.truncated = true;
            return;
        }
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code,
            location: location.into(),
            message: message.into(),
        });
    }
    pub(crate) fn single(
        source: &str,
        code: &'static str,
        location: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        let mut report = Self::new(source, 1);
        report.push(code, location, message);
        report
    }
    pub fn source_name(&self) -> &str {
        &self.source
    }
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
    pub fn truncated(&self) -> bool {
        self.truncated
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }
}

/// Conservative ASCII terminal rendering. Escapes controls, bidi and non-ASCII;
/// caps each field to 512 source characters without cutting a UTF-8 codepoint.
pub fn terminal_safe(value: &str) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if index == 512 {
            out.push_str("...[truncated]");
            break;
        }
        if ch.is_ascii_graphic() || ch == ' ' {
            out.push(ch);
        } else {
            out.extend(ch.escape_default());
        }
    }
    out
}
impl fmt::Display for ContentReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for diagnostic in &self.diagnostics {
            writeln!(
                f,
                "{}:{}\nerror[{}]: {}",
                terminal_safe(&self.source),
                terminal_safe(&diagnostic.location),
                diagnostic.code,
                terminal_safe(&diagnostic.message)
            )?;
        }
        if self.truncated {
            writeln!(f, "Further diagnostics omitted (report limit reached).")?;
        }
        Ok(())
    }
}
impl Error for ContentReport {}

pub(crate) fn pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}
