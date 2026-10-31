//! Decides whether a game session ended normally, from its exit code and the `[ee] stopped` line.
//!
//! Mirrors `external_runtime_probe.summarize_exit` and the manual-session rule in `run_probe`:
//! a normal close is exit code 0 with the reason `window closed`.

const STOP_PREFIX: &str = "[ee] stopped after ";
const CYCLES_MARKER: &str = " cycles: ";
const HOST_FAILURE_PREFIX: &str = "[host] audio/input/window failure: ";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The player closed the window (exit code 0, `window closed`).
    NormalClose,
    /// Anything else. `reason` is the text of the last `[ee] stopped` line or host failure line.
    Crashed { reason: Option<String>, exit_code: Option<i32> },
}

/// Classifies a finished session from its full log text and exit code (`None` = killed by us).
pub fn classify(log_text: &str, exit_code: Option<i32>) -> Outcome {
    let ee_reason = log_text
        .lines()
        .filter_map(stop_reason)
        .last()
        .map(str::to_string);
    if exit_code == Some(0) && ee_reason.as_deref() == Some("window closed") {
        Outcome::NormalClose
    } else {
        let reason = ee_reason.or_else(|| {
            log_text
                .lines()
                .filter_map(host_failure_reason)
                .last()
                .map(str::to_string)
        });
        Outcome::Crashed { reason, exit_code }
    }
}

/// `[ee] stopped after N frames, C cycles: <reason>` -> `<reason>`.
fn stop_reason(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix(STOP_PREFIX)?;
    let index = rest.find(CYCLES_MARKER)?;
    Some(rest[index + CYCLES_MARKER.len()..].trim())
}

/// `[host] audio/input/window failure: <reason>` -> `<reason>`.
fn host_failure_reason(line: &str) -> Option<&str> {
    line.trim().strip_prefix(HOST_FAILURE_PREFIX).map(str::trim)
}

/// One plain sentence for the crash dialog. Technical detail stays in the log.
pub fn user_message(outcome: &Outcome) -> String {
    match outcome {
        Outcome::NormalClose => String::new(),
        Outcome::Crashed { reason: Some(reason), .. } => {
            if reason.contains("waveOutOpen") || reason.contains("A device ID has been used that is out of range") {
                "Gran Turismo 4 could not start because no audio output device was found. Connect headphones or speakers, or enable an audio output device in Windows.".to_string()
            } else {
                format!("Gran Turismo 4 stopped unexpectedly ({reason}). The session folder has the details.")
            }
        }
        Outcome::Crashed { reason: None, exit_code: Some(code) } => {
            format!("Gran Turismo 4 stopped unexpectedly (exit code {code}). The session folder has the details.")
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
    fn host_audio_failure_is_classified_with_actionable_user_message() {
        let log = "[perf] session end: partial interval omitted; use F8 before closing\n\
                   [host] audio/input/window failure: waveOutOpen stereo16/48000: A device ID has been used that is out of range for your system. (2)\n";
        let outcome = classify(log, Some(1));
        assert_eq!(
            outcome,
            Outcome::Crashed {
                reason: Some("waveOutOpen stereo16/48000: A device ID has been used that is out of range for your system. (2)".into()),
                exit_code: Some(1),
            }
        );
        assert_eq!(
            user_message(&outcome),
            "Gran Turismo 4 could not start because no audio output device was found. Connect headphones or speakers, or enable an audio output device in Windows."
        );
    }

    #[test]
    fn other_host_failure_is_classified() {
        let log = "[host] audio/input/window failure: Native Vulkan presentation refused window\n";
        let outcome = classify(log, Some(1));
        assert_eq!(
            outcome,
            Outcome::Crashed {
                reason: Some("Native Vulkan presentation refused window".into()),
                exit_code: Some(1),
            }
        );
        assert_eq!(
            user_message(&outcome),
            "Gran Turismo 4 stopped unexpectedly (Native Vulkan presentation refused window). The session folder has the details."
        );
    }
}
