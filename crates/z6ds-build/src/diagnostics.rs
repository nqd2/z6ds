//! GCC diagnostic line parser for M06 / M07.

use regex::Regex;
use std::sync::LazyLock;

use z6ds_core::contracts::{Diagnostic, SCHEMA_VERSION_BUILD};

static GCC_DIAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<path>[^:]+):(?P<line>\d+):(?P<column>\d+):\s*(?P<severity>error|warning|note):\s*(?P<message>.+)$",
    )
    .expect("gcc diagnostic regex")
});

static GCC_DIAG_NO_COL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<path>[^:]+):(?P<line>\d+):\s*(?P<severity>error|warning|note):\s*(?P<message>.+)$",
    )
    .expect("gcc diagnostic regex (no column)")
});

/// Parse all GCC-style diagnostics from build log text.
pub fn parse_diagnostics(log: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for line in log.lines() {
        if let Some(d) = parse_line(line) {
            out.push(d);
        }
    }
    out
}

fn parse_line(line: &str) -> Option<Diagnostic> {
    let trimmed = line.trim();
    if let Some(caps) = GCC_DIAG.captures(trimmed) {
        return Some(diagnostic(
            caps["path"].trim(),
            caps["line"].parse().ok()?,
            caps["column"].parse().ok()?,
            &caps["severity"],
            caps["message"].trim(),
        ));
    }
    if let Some(caps) = GCC_DIAG_NO_COL.captures(trimmed) {
        return Some(diagnostic(
            caps["path"].trim(),
            caps["line"].parse().ok()?,
            0,
            &caps["severity"],
            caps["message"].trim(),
        ));
    }
    None
}

fn diagnostic(path: &str, line: u32, column: u32, severity: &str, message: &str) -> Diagnostic {
    Diagnostic {
        schema_version: SCHEMA_VERSION_BUILD,
        path: path.to_string(),
        line,
        column,
        severity: severity.to_string(),
        message: message.to_string(),
        source: "gcc".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tc_m06_03_parse_gcc_diagnostic() {
        let log = "Core/Src/main.c:42:5: error: expected ';' before '}' token";
        let diags = parse_diagnostics(log);
        assert_eq!(diags.len(), 1);
        let d = &diags[0];
        assert_eq!(d.path, "Core/Src/main.c");
        assert_eq!(d.line, 42);
        assert_eq!(d.column, 5);
        assert_eq!(d.severity, "error");
        assert!(d.message.contains("expected"));
    }

    #[test]
    fn parses_warning_without_column() {
        let log = "foo.c:10: warning: unused variable";
        let diags = parse_diagnostics(log);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].line, 10);
        assert_eq!(diags[0].column, 0);
        assert_eq!(diags[0].severity, "warning");
    }
}
