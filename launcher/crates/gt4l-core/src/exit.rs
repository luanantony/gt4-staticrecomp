//! Decides whether a game session ended normally, from its exit code and the `[ee] stopped` line.
//!
//! Mirrors `external_runtime_probe.summarize_exit` and the manual-session rule in `run_probe`:
//! a normal close is exit code 0 with the reason `window closed`.

const STOP_PREFIX: &str = "[ee] stopped after ";
const CYCLES_MARKER: &str = " cycles: ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The player closed the window (exit code 0, `window closed`).
    NormalClose,
    /// Anything else. `reason` is the text of the last `[ee] stopped` line, when there was one.
    Crashed { reason: Option<String>, exit_code: Option<i32> },
}

/// Classifies a finished session from its full log text and exit code (`None` = killed by us).
pub fn classify(log_text: &str, exit_code: Option<i32>) -> Outcome {
    let reason = log_text
        .lines()
        .filter_map(stop_reason)
        .last()
        .map(str::to_string);
    if exit_code == Some(0) && reason.as_deref() == Some("window closed") {
        Outcome::NormalClose
    } else {
        Outcome::Crashed { reason, exit_code }
    }
}

/// `[ee] stopped after N frames, C cycles: <reason>` -> `<reason>`.
fn stop_reason(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix(STOP_PREFIX)?;
    let index = rest.find(CYCLES_MARKER)?;
    Some(rest[index + CYCLES_MARKER.len()..].trim())
}

/// One plain sentence for the crash dialog. Technical detail stays in the log.
pub fn user_message(outcome: &Outcome) -> String {
    match outcome {
        Outcome::NormalClose => String::new(),
        Outcome::Crashed { reason: Some(reason), .. } => {
            format!("Gran Turismo 4 stopped unexpectedly ({reason}). The session folder has the details.")
        }
        Outcome::Crashed { reason: None, exit_code: Some(code) } => {
            let ucode = *code as u32;
            match ucode {
                0xC0000135 => "Gran Turismo 4 could not start because a required system component (such as the Microsoft Visual C++ 2015-2022 Redistributable) is missing (exit code 0xC0000135).".to_string(),
                0xCFFFFFFF => "Gran Turismo 4 was closed by Windows because the window stopped responding (exit code 0xCFFFFFFF).".to_string(),
                _ if ucode >= 0x80000000 => {
                    format!("Gran Turismo 4 stopped unexpectedly (exit code 0x{ucode:08X}). The session folder has the details.")
                }
                _ => {
                    format!("Gran Turismo 4 stopped unexpectedly (exit code {code}). The session folder has the details.")
                }
            }
        }
        Outcome::Crashed { reason: None, exit_code: None } => {
            "Gran Turismo 4 stopped unexpectedly. The session folder has the details.".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_closed_with_exit_zero_is_normal() {
        let log = "boot\n[ee] stopped after 1200 frames, 9000 cycles: window closed\n";
        assert_eq!(classify(log, Some(0)), Outcome::NormalClose);
    }

    #[test]
    fn last_stop_line_wins() {
        let log = "[ee] stopped after 5 frames, 1 cycles: window closed\n\
                   [ee] stopped after 9 frames, 2 cycles: no translated code at 0x00123456\n";
        assert_eq!(
            classify(log, Some(1)),
            Outcome::Crashed { reason: Some("no translated code at 0x00123456".into()), exit_code: Some(1) }
        );
    }

    #[test]
    fn window_closed_with_nonzero_exit_is_a_crash() {
        let log = "[ee] stopped after 5 frames, 1 cycles: window closed\n";
        assert!(matches!(classify(log, Some(3)), Outcome::Crashed { exit_code: Some(3), .. }));
    }

    #[test]
    fn missing_stop_line_and_killed_process_is_a_crash_without_reason() {
        assert_eq!(classify("", None), Outcome::Crashed { reason: None, exit_code: None });
    }

    #[test]
    fn user_message_names_the_reason_in_one_sentence() {
        let outcome = Outcome::Crashed { reason: Some("no translated code".into()), exit_code: Some(1) };
        assert!(user_message(&outcome).starts_with("Gran Turismo 4 stopped unexpectedly (no translated code)"));
    }

    #[test]
    fn user_message_interprets_dll_not_found_exit_code() {
        let outcome = Outcome::Crashed { reason: None, exit_code: Some(-1073741515) };
        assert!(user_message(&outcome).contains("Visual C++ 2015-2022 Redistributable"));
        assert!(user_message(&outcome).contains("0xC0000135"));
    }

    #[test]
    fn user_message_interprets_application_hang_exit_code() {
        let outcome = Outcome::Crashed { reason: None, exit_code: Some(-805306369) };
        assert!(user_message(&outcome).contains("window stopped responding"));
        assert!(user_message(&outcome).contains("0xCFFFFFFF"));
    }

    #[test]
    fn user_message_formats_high_exit_codes_as_hex() {
        let outcome = Outcome::Crashed { reason: None, exit_code: Some(-1073741819) };
        assert_eq!(
            user_message(&outcome),
            "Gran Turismo 4 stopped unexpectedly (exit code 0xC0000005). The session folder has the details."
        );
    }
}
