//! Reproducible-build and terminal-output environment conventions.

use std::io::IsTerminal;

use crate::{CliError, CliResult};

const MAX_UNIX_TIMESTAMP: u64 = 253_402_300_799;

pub(crate) fn source_date_epoch_timestamp() -> CliResult<Option<String>> {
    let value = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Err(error) => {
            return Err(CliError::invalid_args(format!(
                "SOURCE_DATE_EPOCH is not valid UTF-8: {error}"
            )));
        }
    };
    let seconds = value
        .parse::<u64>()
        .map_err(|_| CliError::invalid_args("SOURCE_DATE_EPOCH must be a non-negative integer"))?;
    if seconds > MAX_UNIX_TIMESTAMP {
        return Err(CliError::invalid_args(
            "SOURCE_DATE_EPOCH must represent a UTC instant no later than 9999-12-31T23:59:59Z",
        ));
    }
    Ok(Some(format_unix_timestamp(seconds)))
}

pub(crate) fn text_for_stdout(text: String) -> String {
    let environment = TextEnvironment {
        no_color: std::env::var_os("NO_COLOR").is_some(),
        ci: std::env::var_os("CI").is_some(),
        term: std::env::var("TERM").ok(),
        is_terminal: std::io::stdout().is_terminal(),
    };
    text_for_environment(&text, &environment)
}

#[derive(Debug)]
struct TextEnvironment {
    no_color: bool,
    ci: bool,
    term: Option<String>,
    is_terminal: bool,
}

fn text_for_environment(text: &str, environment: &TextEnvironment) -> String {
    if styling_allowed(environment) {
        text.to_string()
    } else {
        strip_ansi_control_sequences(text)
    }
}

fn styling_allowed(environment: &TextEnvironment) -> bool {
    environment.is_terminal
        && !environment.no_color
        && !environment.ci
        && !environment
            .term
            .as_deref()
            .is_some_and(|term| term.eq_ignore_ascii_case("dumb"))
}

fn strip_ansi_control_sequences(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' || characters.peek() != Some(&'[') {
            output.push(character);
            continue;
        }
        characters.next();
        for control in characters.by_ref() {
            if ('@'..='~').contains(&control) {
                break;
            }
        }
    }
    output
}

fn format_unix_timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let day_seconds = seconds % 86_400;
    let hour = day_seconds / 3_600;
    let minute = day_seconds % 3_600 / 60;
    let second = day_seconds % 60;
    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month as u32, day as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(
        no_color: bool,
        ci: bool,
        term: Option<&str>,
        is_terminal: bool,
    ) -> TextEnvironment {
        TextEnvironment {
            no_color,
            ci,
            term: term.map(ToOwned::to_owned),
            is_terminal,
        }
    }

    #[test]
    fn every_automation_convention_strips_styling_but_keeps_unicode_text() {
        let styled = "\u{1b}[1;31mÉchec\u{1b}[0m: bad";
        for environment in [
            environment(true, false, Some("xterm"), true),
            environment(false, true, Some("xterm"), true),
            environment(false, false, Some("dumb"), true),
            environment(false, false, Some("xterm"), false),
        ] {
            assert_eq!(text_for_environment(styled, &environment), "Échec: bad");
            assert!(!styling_allowed(&environment));
        }
        let interactive = environment(false, false, Some("xterm-256color"), true);
        assert!(styling_allowed(&interactive));
        assert_eq!(text_for_environment(styled, &interactive), styled);
    }

    #[test]
    fn unix_timestamp_format_covers_epoch_leap_day_and_upper_bound() {
        assert_eq!(format_unix_timestamp(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix_timestamp(946_684_800), "2000-01-01T00:00:00Z");
        assert_eq!(
            format_unix_timestamp(MAX_UNIX_TIMESTAMP),
            "9999-12-31T23:59:59Z"
        );
    }
}
