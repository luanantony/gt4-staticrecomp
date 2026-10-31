//! The launcher's gamepad service. One thread owns the SDL3 reader (`gt4l_platform::sdl_pads`, the layer the game
//! uses) and turns it into Tauri events for the page:
//!
//! - `pad-status` `{available, connected, name, kind, player_index, count, error}`: on every change. `count` is the
//!   number of pads, and the named pad is "controller 1 of N", the one the game reads (`pad::active_pad`).
//! - `pad-live` `{sources, sticks}`: about 60 times a second while a pad is connected and the window is shown.
//!   `sources` holds the pressure (0..1) of each controls-file source (`xinput:A`, `xinput:LT`...); the page maps
//!   them to the DS2 buttons through the controls file. `sticks.left|right` are -1..1, y positive down.
//! - `pad-nav` `{name}`: a press for nav.js (`a b x y lb rb start`, and `up down left right` with repeat).
//!
//! The thread does no SDL work while the window is hidden for a game (`Handle::set_active(false)`, called from the
//! tray). A rebind capture is answered by the same loop, so the page never reads the pad itself.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use gt4l_platform::pad::{self, PadSample};
use gt4l_platform::sdl_pads::{PadInfo, SdlPads};
use serde_json::{json, Map, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::pad_nav::Navigator;
use crate::state::{self, Shared};

/// About 60 readings a second.
const TICK: Duration = Duration::from_millis(16);
/// The error a capture returns when it is cancelled (the page looks for "cancel" in it).
pub const CAPTURE_CANCELLED: &str = "capture cancelled";

enum Command {
    /// Waits for the next newly pressed control of the active pad; the reply carries its source name.
    Capture(Sender<Result<String, String>>),
    Cancel,
}

struct PendingCapture {
    reply: Sender<Result<String, String>>,
    /// The pad as it was when the capture started (set on the first reading), so held controls do not count.
    baseline: Option<PadSample>,
}

/// The page-facing side of the service, kept in Tauri's state.
#[derive(Clone)]
pub struct Handle {
    commands: Sender<Command>,
    status: Arc<Mutex<Value>>,
    active: Arc<AtomicBool>,
}

impl Handle {
    /// Starts a capture. The receiver gets `Ok(source)` for the control, or `Err` when cancelled or paused.
    pub fn start_capture(&self) -> Receiver<Result<String, String>> {
        let (reply, receiver) = channel();
        if self.commands.send(Command::Capture(reply)).is_err() {
            // The service thread is gone: the dropped sender makes the receiver fail at once.
        }
        receiver
    }

    pub fn cancel_capture(&self) {
        let _ = self.commands.send(Command::Cancel);
    }

    /// False while the window is hidden for a game: the service stops reading the pad.
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::Relaxed);
    }

    /// The last `pad-status` payload (for a page that opens after the event).
    pub fn status(&self) -> Value {
        match self.status.lock() {
            Ok(value) => value.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

/// Starts the service thread and returns its handle. Manage the handle in Tauri's state.
pub fn spawn(app: AppHandle) -> Handle {
    let (commands, receiver) = channel();
    let handle = Handle {
        commands,
        status: Arc::new(Mutex::new(status_json(false, 0, None, "starting"))),
        active: Arc::new(AtomicBool::new(true)),
    };
    let worker = handle.clone();
    let spawned = std::thread::Builder::new()
        .name("gt4-pad".to_string())
        .spawn(move || run(app, receiver, worker));
    if spawned.is_err() {
        // No thread: captures fail at once (the dropped receiver) and the status says so.
        publish(None, &handle, status_json(false, 0, None, "the gamepad thread could not start"));
    }
    handle
}

/// The `pad-status` payload. `available` is false when SDL did not start.
pub fn status_json(available: bool, count: usize, active: Option<&PadInfo>, error: &str) -> Value {
    json!({
        "available": available,
        "connected": active.is_some(),
        "name": active.map(|pad| pad.name.clone()).unwrap_or_default(),
        "kind": active.map(|pad| pad.kind.clone()).unwrap_or_default(),
        "player_index": active.and_then(|pad| pad.player_index),
        "count": count,
        "error": error,
    })
}

/// The `pad-live` payload: the mapped sources and the sticks of one reading.
pub fn live_json(sample: &PadSample) -> Value {
    let sources: Map<String, Value> = sample
        .pressure
        .iter()
        .map(|(name, pressure)| ((*name).to_string(), json!(f64::from(*pressure) / 255.0)))
        .collect();
    json!({
        "sources": sources,
        "sticks": {
            "left": { "x": axis(sample.left_x), "y": axis(sample.left_y) },
            "right": { "x": axis(sample.right_x), "y": axis(sample.right_y) },
        },
    })
}

fn axis(raw: i16) -> f64 {
    (f64::from(raw) / 32_767.0).clamp(-1.0, 1.0)
}

fn publish(app: Option<&AppHandle>, handle: &Handle, status: Value) {
    if let Ok(mut slot) = handle.status.lock() {
        *slot = status.clone();
    }
    if let Some(app) = app {
        let _ = app.emit("pad-status", status);
    }
}

fn log(app: &AppHandle, message: &str) {
    let dirs = {
        let shared = app.state::<Shared>();
        let state = state::lock(shared.inner());
        state.dirs.clone()
    };
    state::log(&dirs, message);
}

fn run(app: AppHandle, commands: Receiver<Command>, handle: Handle) {
    let mut pads = match SdlPads::start() {
        Ok(pads) => pads,
        Err(message) => {
            log(&app, &format!("controller input unavailable: {message}"));
            publish(Some(&app), &handle, status_json(false, 0, None, &message));
            // Keep answering: a capture fails with the reason instead of waiting for a pad that cannot come.
            while let Ok(command) = commands.recv() {
                if let Command::Capture(reply) = command {
                    let _ = reply.send(Err(message.clone()));
                }
            }
            return;
        }
    };
    log(&app, "controller input: SDL3 gamepad subsystem started");

    let mut navigator = Navigator::default();
    let mut capture: Option<PendingCapture> = None;
    let mut last_status = Value::Null;
    let mut was_active = true;

    loop {
        std::thread::sleep(TICK);
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::Capture(reply) => {
                    if let Some(previous) = capture.take() {
                        let _ = previous.reply.send(Err(CAPTURE_CANCELLED.to_string()));
                    }
                    capture = Some(PendingCapture { reply, baseline: None });
                }
                Command::Cancel => {
                    if let Some(pending) = capture.take() {
                        let _ = pending.reply.send(Err(CAPTURE_CANCELLED.to_string()));
                    }
                }
            }
        }

        if !handle.active.load(Ordering::Relaxed) {
            if was_active {
                was_active = false;
                navigator = Navigator::default();
                if let Some(pending) = capture.take() {
                    let _ = pending.reply.send(Err("controller input is paused while the game runs".to_string()));
                }
            }
            continue;
        }
        was_active = true;

        pads.pump();
        let infos = pads.connected();
        let ids: Vec<u32> = infos.iter().map(|pad| pad.id).collect();
        let active_info = pad::active_pad(&ids).and_then(|id| infos.iter().find(|pad| pad.id == id));
        let status = status_json(true, infos.len(), active_info, "");
        if status != last_status {
            publish(Some(&app), &handle, status.clone());
            last_status = status;
        }

        let Some(id) = pad::active_pad(&ids) else {
            navigator = Navigator::default();
            continue;
        };
        let Some(raw) = pads.read(id) else {
            continue;
        };
        let sample = pad::sample(&raw);
        let _ = app.emit("pad-live", live_json(&sample));
        for name in navigator.update(&sample, Instant::now()) {
            let _ = app.emit("pad-nav", json!({ "name": name }));
        }

        let found = match capture.as_mut() {
            None => None,
            Some(pending) => match &pending.baseline {
                None => {
                    pending.baseline = Some(sample.clone());
                    None
                }
                Some(baseline) => pad::captured_source(baseline, &sample),
            },
        };
        if let Some(source) = found {
            if let Some(pending) = capture.take() {
                let _ = pending.reply.send(Ok(source));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gt4l_platform::pad::{sample, RawPad};

    #[test]
    fn live_sources_are_the_controls_file_names_with_pressure_0_to_1() {
        let mut raw = RawPad::default();
        raw.south = true;
        raw.left_trigger = 32_767;
        let live = live_json(&sample(&raw));
        assert_eq!(live["sources"]["xinput:A"], json!(1.0));
        assert_eq!(live["sources"]["xinput:B"], json!(0.0));
        assert_eq!(live["sources"]["xinput:LT"], json!(1.0));
    }

    #[test]
    fn sticks_are_in_minus_one_to_one_with_y_down() {
        let mut raw = RawPad::default();
        raw.left_y = i16::MIN;
        raw.right_x = i16::MAX;
        let live = live_json(&sample(&raw));
        assert_eq!(live["sticks"]["left"]["y"], json!(-1.0), "SDL y is positive down: the most negative raw value is -1");
        assert!((live["sticks"]["right"]["x"].as_f64().unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn status_names_the_active_pad_and_counts_all_of_them() {
        let pad = PadInfo { id: 3, name: "DualSense Wireless Controller".into(), kind: "ps5".into(), player_index: Some(0) };
        let status = status_json(true, 2, Some(&pad), "");
        assert_eq!(status["connected"], json!(true));
        assert_eq!(status["name"], json!("DualSense Wireless Controller"));
        assert_eq!(status["kind"], json!("ps5"));
        assert_eq!(status["count"], json!(2));
        let none = status_json(true, 0, None, "");
        assert_eq!(none["connected"], json!(false));
        assert_eq!(none["name"], json!(""));
    }

    #[test]
    fn an_unavailable_status_carries_the_reason() {
        let status = status_json(false, 0, None, "SDL3 did not start: no device");
        assert_eq!(status["available"], json!(false));
        assert_eq!(status["error"], json!("SDL3 did not start: no device"));
    }
}
