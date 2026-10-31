/*
 * Development mock of the Rust command layer. Loaded only by ui/dev/index.html.
 * It answers the same command names and payloads as the Tauri build, so the
 * pages render in a plain browser. Values are sample values, not game data.
 *
 * Query options (ui/dev/index.html?...):
 *   setup=done|needed|wrong|copying|checking|preparing|renderer|failed|missing|offline   (default done)
 *   pick=spec                                       (setup: the picker returns a Spec II image, refused)
 *   running=1                                       (game running at start)
 *   focus=play                                      (home: focus the Play button, for screenshots)
 *   ra=signedin                                     (Achievements: synthetic account and progress, no network)
 *   Controls: pad=A,LB,DpadRight lt=0.6 rt=0.2 lx=0.7 ly=-0.45 rx= ry= pads=2 nopad=1 (a still dev controller),
 *             select=circle mode=key capture=hold|conflict remap=1 locked=1
 */
(function (root) {
  "use strict";

  var query = new URLSearchParams(root.location.search);
  var setupStart = query.get("setup") || "done";
  var IMAGE_BYTES = 5314478080;
  var SPEC_MESSAGE = "This disc is not supported yet. GT4Recomp supports Gran Turismo 4 (USA, SCUS-97328). " +
    "This looks like Gran Turismo 4 Spec II or GT4 Online (SCUS-97436).";

  var setup = { state: "done", step: "copying", progress: 0, bytes_done: 0, bytes_total: 0, renderer_step: "",
    disc_name: "Gran Turismo 4 \u00b7 USA \u00b7 v2.00", problem: "", note: "" };
  function running(step, progress) {
    setup.state = "checking";
    setup.step = step;
    setup.progress = progress;
    setup.bytes_total = step === "copying" ? IMAGE_BYTES : (step === "renderer" ? 48000000 : 0);
    setup.bytes_done = Math.round(setup.bytes_total * progress);
    setup.renderer_step = step === "renderer" ? "downloading" : "";
  }
  if (setupStart === "wrong") {
    setup.state = "wrong_disc";
    setup.disc_name = "Gran Turismo 4 Spec II v1.10.iso";
    setup.problem = SPEC_MESSAGE;
  } else if (setupStart === "needed") {
    setup.state = "needed";
    setup.disc_name = "";
  } else if (setupStart === "copying" || setupStart === "checking") {
    running("copying", 0.62);
  } else if (setupStart === "preparing") {
    running("preparing", 0.5);
  } else if (setupStart === "renderer") {
    running("renderer", 0.4);
  } else if (setupStart === "failed") {
    setup.state = "failed";
    setup.problem = "Not enough free space to copy the game files: 5.8 GB needed, 2.1 GB free. Free up space and try again.";
  } else if (setupStart === "missing") {
    setup.state = "files_missing";
    setup.disc_name = "";
    setup.problem = "Game files missing (SCUS-97328.iso is missing). Choose your disc image again.";
  } else if (setupStart === "offline") {
    setup.note = "GSdx could not be downloaded, so the game cannot start yet. GT4Recomp tries again at the next start.";
  }

  // setup_start: walk the steps on a timer, like the Rust thread.
  var PLAN = [["copying", 0], ["copying", 0.35], ["copying", 0.7], ["copying", 1], ["checking", 1], ["preparing", 0.5],
    ["renderer", 0.3], ["renderer", 0.8], ["renderer", 1]];
  var setupTimer = null;
  function startSetup() {
    var index = 0;
    clearInterval(setupTimer);
    setupTimer = setInterval(function () {
      if (index >= PLAN.length) {
        clearInterval(setupTimer);
        setup.state = "done";
        setup.renderer_step = "done";
        return;
      }
      running(PLAN[index][0], PLAN[index][1]);
      index += 1;
    }, 500);
  }

  var gameRunning = query.get("running") === "1";

  // Read by pages/home.js: focuses Play on mount. Only the dev harness sets it.
  root.GT4 = root.GT4 || {};
  root.GT4.devFocus = query.get("focus") || "";
  var maxUpscale = 4;

  // Settings rows, the same shape as the Rust table.
  var DEFAULTS = {};
  var rows = [
    { id: "renderer", label: "Renderer", help: "GSdx draws the game on your GPU in its own process; it is the renderer GT4Recomp uses. CoreGS 2.0 is in development.",
      page: "basic", group: "graphics", control: "dropdown",
      values: [
        { value: "pcsx2", label: "GSdx" },
        { value: "coregs-2.0", label: "CoreGS 2.0 (coming soon)", disabled: true }
      ],
      default: "pcsx2", current: "pcsx2" },
    // TASK-080 addendum: no Aspect ratio and no Internal resolution row. Render scale (50% to 200%) with the
    // resulting internal resolution as its readout (labels are filled by tableView from the chosen resolution).
    { id: "render_scale", label: "Render scale", help: "How sharp the picture is drawn. 100% draws it at least at your resolution; 50% is faster and softer, 200% is sharper and needs a much faster GPU.",
      page: "basic", group: "graphics", control: "slider",
      values: { min: 50, max: 200, step: 25, unit: "%" },
      default: 100, current: 100 },
    { id: "fxaa", label: "Anti-aliasing (FXAA)", help: "Smooths jagged edges with a fast filter. It softens fine detail a little. Off by default.",
      page: "basic", group: "graphics", control: "toggle", values: null, default: false, current: false },
    { id: "performance_mode", label: "Performance Mode", help: "Faster on weaker PCs. Some effects that read the image back are skipped, such as headlight and sun flares and the car icons next to the gap time. Applies the next time you play.",
      page: "basic", group: "graphics", control: "toggle", values: null, default: false, current: false },
    { id: "cas_mode", label: "Sharpening (CAS)", help: "FidelityFX Contrast Adaptive Sharpening. Off leaves the picture as the game drew it.",
      page: "basic", group: "graphics", control: "dropdown",
      values: [
        { value: "off", label: "Off" },
        { value: "sharpen-only", label: "Sharpen only" },
        { value: "sharpen-and-resize", label: "Sharpen and resize" }
      ],
      default: "off", current: "off" },
    { id: "cas_sharpness", label: "Sharpening amount", help: "How strong the CAS sharpening is. Used when CAS is on.",
      page: "basic", group: "graphics", control: "slider",
      values: { min: 0, max: 100, step: 1, unit: "%" },
      default: 50, current: 50 },
    { id: "monitor", label: "Monitor", help: "The screen the game opens on. The primary monitor is used until you pick another one.",
      page: "basic", group: "display", control: "dropdown", values: [], default: "primary", current: "primary" },
    { id: "resolution", label: "Resolution", help: "The size of the picture, from the sizes your monitor supports. The shape of the screen follows it.",
      page: "basic", group: "display", control: "dropdown", values: [], default: "desktop", current: "desktop" },
    { id: "fullscreen", label: "Fullscreen", help: "Start in fullscreen. Alt+Enter or F11 switches while playing.",
      page: "basic", group: "display", control: "toggle", values: null, default: false, current: true },
    { id: "vsync", label: "VSync", help: "On keeps motion smooth with no tearing. Off lowers latency and may tear.",
      page: "basic", group: "display", control: "dropdown",
      values: [
        { value: "on", label: "On" },
        { value: "off", label: "Off" }
      ],
      default: "on", current: "on" },
    { id: "async_gs", label: "Asynchronous GS", help: "Sends graphics work to the GPU without waiting. Needed for full speed; turn off only to compare.",
      page: "advanced", control: "toggle", values: null, default: true, current: true },
    { id: "unsynced_readbacks", label: "Unsynced readbacks", help: "Faster, but effects that read the screen will be wrong.",
      page: "advanced", control: "toggle", values: null, default: false, current: false },
    { id: "performance_report", label: "Performance report", help: "Records frame timing while you play for bug reports.",
      page: "advanced", control: "toggle", values: null, default: false, current: false },
    { id: "perf_overlay", label: "Performance overlay", help: "Shows GT4Recomp's own overlay while you play (FPS, frame time, CPU and GPU use).",
      page: "advanced", control: "toggle", values: null, default: false, current: false },
    { id: "perf_overlay_fps", label: "Overlay: FPS", help: "Shows the frame rate in the performance overlay.",
      page: "advanced", control: "toggle", values: null, default: true, current: true },
    { id: "debug_internal", label: "Internal debug", help: "Not shown to users.",
      page: "hidden", control: "toggle", values: null, default: false, current: true },
    // TASK-091 (1.2.1): the launcher's own row, appended by the Rust table (commands::get_settings_table).
    { id: "launcher_gpu", label: "Launcher GPU acceleration", help: "Off keeps the launcher window without GPU acceleration, after it failed to draw in WebView2. On takes effect at the next start of the launcher.",
      page: "advanced", control: "toggle", values: null, default: true, current: true }
  ];
  rows.forEach(function (row) {
    DEFAULTS[row.id] = row.default;
  });

  // Two sample monitors (the Rust table lists the connected ones; see model::settings_table_with_monitors).
  var MONITORS = [
    { id: "\\\\.\\DISPLAY1@1920x1080", label: "TV \u2013 1920x1080 (primary)", desktop: "1920x1080",
      modes: ["1920x1080", "1600x900", "1280x720", "1024x768", "800x600"] },
    { id: "\\\\.\\DISPLAY2@3440x1440", label: "DELL U3419W \u2013 3440x1440", desktop: "3440x1440",
      modes: ["3440x1440", "2560x1080", "1920x1080", "1680x1050", "1280x720"] }
  ];
  // The rule of gt4l_core::display::display_plan: k = ceil(max(W / 640, H / 448)), times the scale, ties up, 1..12.
  function renderLabels(output) {
    var size = output.split("x").map(Number);
    var k = Math.max(Math.ceil(size[0] / 640), Math.ceil(size[1] / 448));
    var labels = {};
    for (var step = 50; step <= 200; step += 25) {
      var m = Math.min(12, Math.max(1, Math.floor((2 * k * step + 100) / 200)));
      labels[String(step)] = step + "% \u00B7 " + m + "x \u00B7 " + (640 * m) + "x" + (448 * m);
    }
    return labels;
  }
  function fillDisplayRows() {
    var monitorRow = findRow("monitor");
    var chosen = MONITORS.filter(function (m) { return m.id === monitorRow.current; })[0] || MONITORS[0];
    monitorRow.current = chosen.id;
    monitorRow.values = MONITORS.map(function (m) { return { value: m.id, label: m.label }; });
    var resolutionRow = findRow("resolution");
    if (chosen.modes.indexOf(resolutionRow.current) < 0) {
      resolutionRow.current = chosen.desktop;
    }
    resolutionRow.default = chosen.desktop;
    resolutionRow.values = chosen.modes.map(function (mode) {
      return { value: mode, label: mode === chosen.desktop ? mode + " (desktop)" : mode };
    });
    findRow("render_scale").values.labels = renderLabels(resolutionRow.current);
  }

  function tableView() {
    fillDisplayRows();
    return {
      max_upscale: maxUpscale,
      rows: rows.map(function (row) {
        return JSON.parse(JSON.stringify(row));
      })
    };
  }

  function findRow(id) {
    for (var i = 0; i < rows.length; i++) {
      if (rows[i].id === id) {
        return rows[i];
      }
    }
    return null;
  }

  // Demo offers for the update popup (?update=1). Notes are sample text, not a real release.
  var DEMO_OFFERS = [
    { stream: "renderer", installed: "1.1.0", latest: "1.1.1", update: true,
      package: "gt4-gs-1.1.1-windows-x64.zip",
      notes: "## gt4-gs 1.1.1\n\n- **Faster** present on VSync Off.\n- Fixes a crash on exit when the game ends first.\n- Sample text for the launcher preview."},
    { stream: "launcher", installed: "1.1.0", latest: "1.1.1", update: true,
      package: "GT4Recomp-Setup-1.1.1.exe",
      notes: "GT4Recomp 1.1.1\n\n* Tray: the launcher comes back when the game ends.\n* Sample text for the launcher preview." }
  ];
  // Controls: the built-in mapping of docs/CONTROLS.md, with the same assign/swap rules as gt4controls (Rust).
  var CONTROL_ORDER = ["dpad_up", "dpad_right", "dpad_down", "dpad_left", "cross", "circle", "square", "triangle",
    "l1", "r1", "l2", "r2", "l3", "r3", "select", "start"];
  var CONTROL_NAMES = { dpad_up: "D-pad up", dpad_right: "D-pad right", dpad_down: "D-pad down", dpad_left: "D-pad left",
    cross: "Cross", circle: "Circle", square: "Square", triangle: "Triangle", l1: "L1", r1: "R1", l2: "L2", r2: "R2",
    l3: "L3", r3: "R3", select: "Select", start: "Start" };
  var DEFAULT_SOURCES = {
    dpad_up: ["xinput:DpadUp", "key:Up"], dpad_right: ["xinput:DpadRight", "key:Right"],
    dpad_down: ["xinput:DpadDown", "key:Down"], dpad_left: ["xinput:DpadLeft", "key:Left"],
    cross: ["xinput:A", "key:Z"], circle: ["xinput:B", "key:X"], square: ["xinput:X", "key:A"], triangle: ["xinput:Y", "key:S"],
    l1: ["xinput:LB", "key:Q"], r1: ["xinput:RB", "key:W"], l2: ["xinput:LT", "key:1"], r2: ["xinput:RT", "key:2"],
    l3: ["xinput:LThumb", "key:C"], r3: ["xinput:RThumb", "key:V"], select: ["xinput:Back", "key:Backspace"],
    start: ["xinput:Start", "key:Enter"]
  };
  var RESERVED = { F7: true, F8: true, F9: true, F11: true };
  var bindings = JSON.parse(JSON.stringify(DEFAULT_SOURCES));
  // ?remap=1: a few changed buttons, so the "Changed" state shows in screenshots.
  if (query.get("remap") === "1") {
    bindings.cross = ["xinput:B", "key:Z"];
    bindings.circle = ["xinput:A", "key:X"];
    bindings.select = ["xinput:Back"];
  }

  function analogDefaults() {
    var curve = [
      { value: "linear", label: "Linear" },
      { value: "s_curve", label: "S-Curve" },
      { value: "smooth_power", label: "Smooth Power" },
      { value: "quadratic", label: "Quadratic" },
      { value: "cubic", label: "Cubic" },
      { value: "sublinear", label: "Sublinear" },
      { value: "logarithmic", label: "Logarithmic" }
    ];
    function slider(id, label, def) {
      return { id: id, label: label, help: "How far it must travel before it counts, in percent of its travel.",
        page: "basic", group: "controls", control: "slider", values: { min: 0, max: 95, step: 1, unit: "%" },
        default: def, current: def };
    }
    function dropdown(id, label, values) {
      return { id: id, label: label, help: "How the pressure grows with the trigger travel.", page: "basic",
        group: "controls", control: "dropdown", values: values || curve, default: (values || curve)[0].value, current: (values || curve)[0].value };
    }
    return [slider("l2_deadzone", "L2 deadzone", 11.8), dropdown("l2_curve", "L2 curve", curve),
      slider("r2_deadzone", "R2 deadzone", 11.8), dropdown("r2_curve", "R2 curve", curve),
      slider("lstick_deadzone", "Left stick deadzone", 24), slider("rstick_deadzone", "Right stick deadzone", 26.5)];
  }
  var analog = analogDefaults();

  function base(source) { return String(source).split("|")[0].trim(); }
  function familyOf(source) { return base(source).split(":")[0]; }
  function nameOf(source) { return base(source).split(":")[1] || ""; }

  function controlsView() {
    return {
      controls: CONTROL_ORDER.map(function (id) {
        var sources = bindings[id].slice();
        var pad = sources.filter(function (s) { return familyOf(s) === "xinput"; })[0];
        var key = sources.filter(function (s) { return familyOf(s) === "key"; })[0];
        return { id: id, name: CONTROL_NAMES[id], binding: sources.length ? nameOf(sources[0]) : "-", sources: sources,
          xinput: pad ? nameOf(pad) : null, key: key ? nameOf(key) : null,
          is_default: JSON.stringify(sources) === JSON.stringify(DEFAULT_SOURCES[id]) };
      }),
      triggers: JSON.parse(JSON.stringify(analog)),
      locked: query.get("locked") === "1"
    };
  }

  function ordered(list) {
    return list.filter(function (s) { return familyOf(s) === "xinput"; }).concat(list.filter(function (s) { return familyOf(s) !== "xinput"; }));
  }

  // The rules of gt4controls::Controls::assign: same family replaced, swap with the holder, refusal without swap.
  function assignBinding(id, source, swap) {
    var b = base(source);
    if (!bindings[id]) {
      throw new Error("'" + id + "' is not a button input");
    }
    if (familyOf(b) === "key" && RESERVED[nameOf(b)]) {
      throw new Error("key '" + nameOf(b) + "' is reserved by the launcher");
    }
    var current = bindings[id];
    if (current.some(function (s) { return base(s) === b; })) {
      var same = controlsView();
      same.swapped_with = null;
      same.unchanged = true;
      return same;
    }
    var previous = current.filter(function (s) { return familyOf(s) === familyOf(b); })[0] || null;
    var holder = CONTROL_ORDER.filter(function (other) {
      return other !== id && bindings[other].some(function (s) { return base(s) === b; });
    })[0] || null;
    if (holder) {
      if (!swap) {
        throw new Error("'" + b + "' is already bound to " + holder);
      }
      var replaced = [];
      bindings[holder].forEach(function (s) {
        var incoming = base(s) === b ? previous : s;
        if (incoming && !replaced.some(function (kept) { return base(kept) === base(incoming); })) {
          replaced.push(incoming);
        }
      });
      bindings[holder] = ordered(replaced);
    }
    bindings[id] = ordered(current.filter(function (s) { return familyOf(s) !== familyOf(b); }).concat([b]));
    var view = controlsView();
    view.swapped_with = holder;
    view.unchanged = false;
    return view;
  }

  // Dev-only live input for the Controls page: ?pad=A,LB,DpadRight&lt=0.6&rt=0&lx=0.7&ly=-0.45&rx=-0.3&ry=0.6&pads=2
  // builds a still Gamepad-like object (standard mapping). ?nopad=1 shows "No controller".
  var STANDARD = ["A", "B", "X", "Y", "LB", "RB", "LT", "RT", "Back", "Start", "LThumb", "RThumb", "DpadUp", "DpadDown",
    "DpadLeft", "DpadRight"];
  function devPad(index) {
    var pressed = (query.get("pad") || "").split(",").filter(Boolean);
    var buttons = STANDARD.map(function (name) {
      var down = pressed.indexOf(name) >= 0;
      var value = down ? 1 : 0;
      if (name === "LT" && query.get("lt")) { value = Number(query.get("lt")); }
      if (name === "RT" && query.get("rt")) { value = Number(query.get("rt")); }
      return { pressed: value > 0.12, value: value };
    });
    var axes = ["lx", "ly", "rx", "ry"].map(function (key) { return Number(query.get(key) || 0); });
    return { id: "Xbox Wireless Controller (XInput STANDARD GAMEPAD)", index: index, connected: true,
      mapping: "standard", buttons: buttons, axes: axes };
  }
  if (query.get("pad") !== null || query.get("lt") !== null || query.get("lx") !== null || query.get("pads") !== null) {
    var devPads = [devPad(0)];
    if (query.get("pads") === "2") {
      var second = devPad(1);
      second.id = "Controller (Gamepad for Xbox 360)";
      devPads.push(second);
    }
    root.GT4.devGamepads = function () { return devPads; };
  } else if (query.get("nopad") === "1") {
    root.GT4.devGamepads = function () { return []; };
  }
  // ?select=circle&mode=key&capture=hold|conflict (Controls screenshots; capture starts once the page is entered).
  root.GT4.devControls = {
    select: query.get("select") || "",
    mode: query.get("mode") || "",
    capture: query.get("capture") || ""
  };

  var pendingCapture = null;
  function captureBinding() {
    return new Promise(function (resolve, reject) {
      pendingCapture = reject;
      var mode = root.GT4.devControls.capture;
      if (mode === "hold") {
        return; // waits until cancel_capture
      }
      setTimeout(function () {
        if (pendingCapture === reject) {
          pendingCapture = null;
          resolve({ control: "", source: mode === "conflict" ? "xinput:LB" : "xinput:RB" });
        }
      }, mode === "conflict" ? 200 : 2000);
    });
  }

  var saves = [
    { id: "01", name: "Save 01", modified: "2026-10-16T06:31:00", active: true },
    { id: "02", name: "Save 02", modified: "2026-10-12T22:14:00", active: false },
    { id: "03", name: "Save 03", modified: "2026-09-03T19:02:00", active: false }
  ];
  var replays = [
    { id: "01", name: "Replay 01", modified: "2026-10-16T21:40:00" },
    { id: "02", name: "Replay 02", modified: "2026-10-16T20:55:00" },
    { id: "03", name: "Replay 03", modified: "2026-10-12T22:31:00" }
  ];

  function appState() {
    return { setup: JSON.parse(JSON.stringify(setup)), game_running: gameRunning,
      renderer_installed: true,
      renderer_version: "1.2.1",
      game_files: "C:\\Program Files\\GT4Recomp\\data\\disc",
      migration: { running: false, item: "", bytes_done: 0, bytes_total: 0, notice: "", problem: false } };
  }

  // Synthetic RetroAchievements data belongs only to this dev backend; it never contacts a server.
  var raGame = {
    game_id: 20580, title: "Gran Turismo 4", total_achievements: 3, unlocked_achievements: 2,
    total_points: 35, unlocked_points: 15,
    achievements: [
      { id: 1001, title: "National B License", description: "Acquire the National B License.", points: 10,
        unlocked: true, unlocked_hardcore: true, badge_name: "", type_label: "Progression" },
      { id: 1002, title: "First Victory", description: "Win your first race.", points: 5,
        unlocked: true, unlocked_hardcore: false, badge_name: "", type_label: "" },
      { id: 1003, title: "Championship Champion", description: "Win the championship.", points: 20,
        unlocked: false, unlocked_hardcore: false, badge_name: "", type_label: "" }
    ]
  };
  var raState = { enabled: true, hardcore: false, encore: false, signedIn: query.get("ra") === "signedin",
    username: "", displayName: "", score: 0, scoreSoftcore: 0, game: null };
  function raSignIn(username) {
    raState.signedIn = true;
    raState.username = raState.displayName = username;
    raState.score = 1250;
    raState.scoreSoftcore = 400;
    raState.game = raGame;
  }
  if (raState.signedIn) raSignIn("DemoDriver");

  function invoke(command, args) {
    args = args || {};
    if (command === "ra_login") {
      if (!args.username || !args.password) throw new Error("Enter a mock username and password.");
      raSignIn(args.username);
    } else if (command === "ra_logout") {
      raState.signedIn = false;
      raState.username = raState.displayName = "";
      raState.score = raState.scoreSoftcore = 0;
      raState.game = null;
    } else if (command === "ra_set_options") {
      raState.enabled = !!args.enabled;
      raState.hardcore = !!args.hardcore;
      raState.encore = !!args.encore;
    }
    if (command === "ra_status" || command === "ra_login" || command === "ra_logout" || command === "ra_set_options") {
      return JSON.parse(JSON.stringify(raState));
    }
    switch (command) {
      case "app_state":
        return appState();
      case "setup_start":
        running("copying", 0);
        startSetup();
        return null;
      case "setup_choose_disc":
        if (query.get("pick") === "spec") {
          setup.state = "wrong_disc";
          setup.disc_name = "Gran Turismo 4 Spec II v1.10.iso";
          setup.problem = SPEC_MESSAGE;
          throw new Error(SPEC_MESSAGE);
        }
        setup.disc_name = "Gran Turismo 4 (USA) (v2.00).iso";
        setup.problem = "";
        return { path: "D:\\Games\\Gran Turismo 4 (USA) (v2.00).iso", serial: "SCUS-97328", version: "2.00",
          name: "Gran Turismo 4 \u00b7 USA \u00b7 v2.00" };
      case "setup_cancel":
        clearInterval(setupTimer);
        setup.state = "failed";
        setup.problem = "Setup was cancelled. Nothing was changed.";
        return null;
      case "play":
        gameRunning = true;
        return null;
      case "stop_game":
        gameRunning = false;
        return null;
      case "get_settings_table":
        return tableView();
      case "set_setting": {
        var row = findRow(args.id) || analog.filter(function (item) { return item.id === args.id; })[0];
        if (!row) {
          throw new Error("Unknown setting " + args.id);
        }
        row.current = args.value;
        if (args.id === "monitor") {
          // As the launcher: another monitor's Resolution goes back to its desktop mode.
          findRow("resolution").current = "desktop";
        }
        // The real set_setting returns nothing; the UI reads the table again.
        return null;
      }
      case "reset_gs_settings_to_defaults":
        rows.forEach(function (row) {
          if (row.page === "basic" || row.page === "advanced") {
            row.current = row.default;
          }
        });
        return tableView();
      case "pad_status":
        return padStatusPayload();
      case "get_controls":
        return controlsView();
      case "capture_binding":
        return captureBinding();
      case "cancel_capture":
        if (pendingCapture) {
          var rejectCapture = pendingCapture;
          pendingCapture = null;
          rejectCapture(new Error("capture cancelled"));
        }
        return null;
      case "assign_binding":
        return assignBinding(args.control, args.source, !!args.swap);
      case "reset_binding":
        bindings[args.control] = DEFAULT_SOURCES[args.control].slice();
        return null;
      case "reset_all_bindings":
        bindings = JSON.parse(JSON.stringify(DEFAULT_SOURCES));
        analog = analogDefaults();
        return controlsView();
      case "saves_list":
        return { items: JSON.parse(JSON.stringify(saves)), folder: "C:\\Users\\Player\\Documents\\Gran Turismo 4\\Saves" };
      case "saves_import":
        return null;
      case "saves_activate":
        saves.forEach(function (save) {
          save.active = save.id === args.id;
        });
        return { items: JSON.parse(JSON.stringify(saves)) };
      case "saves_delete":
        saves = saves.filter(function (save) {
          return save.id !== args.id;
        });
        return { items: JSON.parse(JSON.stringify(saves)) };
      case "replays_list":
        return { items: JSON.parse(JSON.stringify(replays)), folder: "C:\\Users\\Player\\Documents\\Gran Turismo 4\\Replays" };
      case "replays_import":
        return null;
      case "replays_delete":
        replays = replays.filter(function (replay) {
          return replay.id !== args.id;
        });
        return { items: JSON.parse(JSON.stringify(replays)) };
      case "about_info":
        return {
          version: "1.1.0",
          gt4recomp: "1.1.0",
          gt4gs: "1.1.0",
          disc_path: "D:\\Games\\PS2\\Gran Turismo 4 (USA) (v2.00).iso",
          game_files: "C:\\Program Files\\GT4Recomp\\data\\disc",
          data_folder: "C:\\Program Files\\GT4Recomp\\data",
          saves_folder: "C:\\Users\\Player\\AppData\\Roaming\\GT4Recomp"
        };
      case "open_log_folder":
      case "open_game_files_folder":
      case "open_player_folder":
      case "open_saves_folder":
      case "open_replays_folder":
      case "open_textures_folder":
        return null;
      case "check_updates":
        // Demo: ?update=1 offers both streams; otherwise nothing is new.
        if (query.get("update") === "1") {
          return {
            offers: DEMO_OFFERS.map(function (offer) { return JSON.parse(JSON.stringify(offer)); }),
            errors: [], upToDate: false, launcher: "1.1.0", renderer: "1.1.0"
          };
        }
        return { offers: [], errors: [], upToDate: true, launcher: "1.1.0", renderer: "1.1.0" };
      case "apply_update":
        return { ok: true, message: args.stream === "launcher" ? "GT4Recomp setup started." : "gt4-gs 1.1.1 is ready." };
      case "install_renderer_archive":
        return { cancelled: false, version: "1.2.1", message: "gt4-gs 1.2.1 is ready." };
      case "skip_update":
      case "confirm_close":
      case "ui_ready":
        return null;
      default:
        throw new Error("Mock has no command " + command);
    }
  }

  // Events: pages listen through GT4.backend.listen; a demo offer is emitted after load with ?update=1.
  var eventHandlers = {};
  function listen(name, handler) {
    (eventHandlers[name] = eventHandlers[name] || []).push(handler);
  }
  function emit(name, payload) {
    (eventHandlers[name] || []).forEach(function (handler) { handler(payload); });
  }
  // Pad events in the shape of the Rust pad service: pad-status on change, pad-live every 50 ms while a pad is
  // connected. The source is the dev Gamepad-like object (?pad=, ?pads=, ?nopad=); the app itself never reads one.
  function devSummary() {
    var pads = root.GT4.devGamepads ? root.GT4.devGamepads() : [];
    return root.GT4.controlsModel.padSummary(pads);
  }
  function padStatusPayload() {
    var summary = devSummary();
    return { available: true, connected: summary.connected, name: summary.connected ? summary.name : "",
      kind: summary.connected ? "xbox360" : "", player_index: summary.connected ? 0 : null, count: summary.count, error: "" };
  }
  var lastPadStatus = "";
  setInterval(function () {
    if (!root.GT4.controlsModel) {
      return;
    }
    var status = padStatusPayload();
    var key = JSON.stringify(status);
    if (key !== lastPadStatus) {
      lastPadStatus = key;
      emit("pad-status", status);
    }
    var summary = devSummary();
    if (summary.connected) {
      emit("pad-live", root.GT4.controlsModel.padLive(summary.pad));
    }
  }, 50);
  root.GT4_MOCK = { invoke: invoke, listen: listen, emit: emit };
  if (query.get("update") === "1") {
    setTimeout(function () { emit("update-available", { offers: DEMO_OFFERS }); }, 900);
  }
})(window);
