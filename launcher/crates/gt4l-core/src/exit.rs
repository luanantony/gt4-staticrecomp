//! Decides whether a game session ended normally, from its exit code and the `[ee] stopped` line.
//!
//! Mirrors `external_runtime_probe.summarize_exit` and the manual-session rule in `run_probe`:
//! a normal close is exit code 0 with the reason `window closed`.
use crate::language::Language;
use crate::messages;


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
    // Exit code 0 with no shutdown line at all: the log was read before the game flushed it (owner, 2026-10-26: three
    // normal closes were reported as crashes). The caller retries the read, so a real crash, which exits non-zero or
    // prints a reason, is still a crash.
    if exit_code == Some(0) && (reason.as_deref() == Some("window closed") || reason.is_none()) {
        Outcome::NormalClose
    } else {
        Outcome::Crashed { reason, exit_code }
    }
}

/// True when the log holds the game's final shutdown line (`[ee] stopped after ...`). The launcher waits for it before
/// classifying, so a normal close is never read mid-write.
pub fn is_settled(log_text: &str) -> bool {
    log_text.lines().any(|line| stop_reason(line).is_some())
}

/// `[ee] stopped after N frames, C cycles: <reason>` -> `<reason>`.
fn stop_reason(line: &str) -> Option<&str> {
    let rest = line.trim().strip_prefix(STOP_PREFIX)?;
    let index = rest.find(CYCLES_MARKER)?;
    Some(rest[index + CYCLES_MARKER.len()..].trim())
}

/// Text that GSdx writes to a session's `game.log` when the graphics card stops responding (issue #89). The first
/// two are the GSdx assertion and its message ("Host GPU lost too many times, device is probably completely wedged");
/// the last is the Vulkan result name. Matched case-insensitively. A bare "device lost" is not a marker: other devices
/// (audio) can report it too.
pub const GPU_LOST_MARKERS: [&str; 3] = ["host gpu lost", "device is probably completely wedged", "vk_error_device_lost"];

/// The first line of a session log that says the graphics card was lost, or `None`. The line stays in the launcher log
/// as the detail behind the plain message.
pub fn gpu_lost_line(log_text: &str) -> Option<&str> {
    log_text.lines().find(|line| {
        let lower = line.to_lowercase();
        GPU_LOST_MARKERS.iter().any(|marker| lower.contains(marker))
    })
}

/// The plain message for a lost graphics card, in `language` (issue #89). Shown in place of the generic crash text.
pub fn gpu_lost_message_in(language: Language) -> String {
    messages::text("game.gpu_lost", language).to_string()
}

/// One plain sentence for the crash dialog, in English. Technical detail stays in the log.
pub fn user_message(outcome: &Outcome) -> String {
    user_message_in(outcome, Language::fallback())
}

/// [`user_message`] in `language`.
pub fn user_message_in(outcome: &Outcome, language: Language) -> String {
    match outcome {
        Outcome::NormalClose => String::new(),
        Outcome::Crashed { reason: Some(reason), .. } => {
            messages::render("game.stopped_unexpectedly_reason", language, &[("reason", reason)])
        }
        Outcome::Crashed { reason: None, exit_code: Some(code) } => {
            messages::render("game.stopped_unexpectedly_code", language, &[("code", &code.to_string())])
        }
        Outcome::Crashed { reason: None, exit_code: None } => messages::text("game.stopped_unexpectedly", language).to_string(),
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
    fn exit_zero_without_a_shutdown_line_is_normal_not_a_crash() {
        // Owner (2026-10-26): the launcher read the log before the game wrote its last line, so a normal close was
        // reported as Crashed { reason: None, exit_code: Some(0) } three times.
        assert_eq!(classify("boot\n[perf] last line not flushed yet\n", Some(0)), Outcome::NormalClose);
        assert!(!is_settled("boot\n[audio] paced: ...\n"));
        assert!(is_settled("boot\n[ee] stopped after 120 frames, 9000 cycles: window closed\n"));
        // A killed process (no exit code) is still a crash, and so is a non-zero exit.
        assert_eq!(classify("boot\n", None), Outcome::Crashed { reason: None, exit_code: None });
        assert!(matches!(classify("boot\n", Some(1)), Outcome::Crashed { exit_code: Some(1), .. }));
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
    fn the_gsdx_device_lost_text_is_found_in_the_session_log() {
        let log = "[gs] init renderer Vulkan
Assertion failed in function GSRenderer::BeginPresentFrame: Host GPU lost too many times, device is probably completely wedged
[ee] stopped after 120 frames, 9000 cycles: GS process exited";
        assert_eq!(
            gpu_lost_line(log),
            Some("Assertion failed in function GSRenderer::BeginPresentFrame: Host GPU lost too many times, device is probably completely wedged")
        );
        assert!(gpu_lost_line("vkQueueSubmit returned VK_ERROR_DEVICE_LOST").is_some());
        assert!(gpu_lost_line("HOST GPU LOST").is_some(), "matched case-insensitively");
    }

    #[test]
    fn ordinary_crashes_and_other_device_messages_are_not_a_lost_card() {
        assert_eq!(gpu_lost_line(""), None);
        assert_eq!(gpu_lost_line("[ee] stopped after 5 frames, 80 cycles: no translated code at 0x00123456"), None);
        assert_eq!(gpu_lost_line("audio device lost, switching to the default output"), None);
    }

    #[test]
    fn the_gpu_lost_message_is_plain_and_translated() {
        let english = gpu_lost_message_in(Language::fallback());
        assert_eq!(
            english,
            "Your graphics card stopped responding. Try a lower resolution (Graphics > Render scale) or switch Graphics API to Direct3D 11."
        );
        let portuguese = gpu_lost_message_in(Language::from_tag("pt-BR").expect("pt-BR is listed"));
        assert!(portuguese.starts_with("Sua placa de vídeo parou de responder."), "{portuguese}");
        assert!(portuguese.contains("Direct3D 11"), "{portuguese}");
    }
}
