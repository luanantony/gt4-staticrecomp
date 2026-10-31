/*
 * Home: cover art, logo, Play, Settings gear. While the game runs, Play
 * becomes "Running · Stop". Polls app_state for game_running.
 * Commands: app_state, play, stop_game.
 * Controller: A or Start = Play, Y = Settings. Hints show only once a pad is used.
 * Play is glass when idle (no focus): it takes the solid white state only when focused or hovered.
 */
(function (root) {
  "use strict";
  var GT4 = (root.GT4 = root.GT4 || {});
  var h = GT4.ui.h;
  var POLL_MS = 1000;

  function call(command, args) {
    return GT4.backend.call(command, args);
  }

  var GEAR_PATH = "M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z";

  function gearIcon() {
    var svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("width", "22");
    svg.setAttribute("height", "22");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("fill", "none");
    svg.setAttribute("stroke", "currentColor");
    svg.setAttribute("stroke-width", "1.5");
    svg.setAttribute("aria-hidden", "true");
    var circle = document.createElementNS("http://www.w3.org/2000/svg", "circle");
    circle.setAttribute("cx", "12");
    circle.setAttribute("cy", "12");
    circle.setAttribute("r", "3");
    var path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("d", GEAR_PATH);
    svg.appendChild(circle);
    svg.appendChild(path);
    return svg;
  }

  GT4.pages = GT4.pages || {};
  GT4.pages.home = {
    mount: function (root) {
      var page = h("div", { class: "stage home" });
      var playButton = h("button", { class: "play", type: "button" }, "Play");
      var stopButton = h("button", { class: "btn", type: "button" }, "Stop");
      var stateText = h("span", { class: "state", text: "Running" });
      var running = h("div", { class: "running" }, [h("span", { class: "dot" }), stateText, stopButton]);
      var timer = null;
      var isRunning = false;

      function paint(nowRunning) {
        isRunning = nowRunning;
        page.classList.toggle("is-running", nowRunning);
      }

      function promptGsdxInstall() {
        if (GT4.ui.topDialog()) {
          return;
        }
        GT4.ui.openDialog({
          title: "GSdx renderer required",
          text: "Gran Turismo 4 requires the GSdx renderer (gt4-gs.exe) to run. Would you like to download it now or install it from a downloaded zip archive?",
          dismissable: true,
          buttons: [
            {
              label: "Download GSdx",
              primary: true,
              run: function () {
                GT4.ui.toast("Downloading GSdx renderer...");
                call("apply_update", { stream: "renderer" }).then(function (res) {
                  GT4.ui.toast(res && res.message ? res.message : "GSdx installed successfully.");
                }, function (err) {
                  GT4.ui.toast(GT4.errorText(err), true);
                });
              }
            },
            {
              label: "Install from zip...",
              run: function () {
                call("install_renderer_archive").then(function (res) {
                  if (res && res.cancelled) {
                    return;
                  }
                  GT4.ui.toast(res && res.message ? res.message : "GSdx installed successfully.");
                }, function (err) {
                  GT4.ui.toast(GT4.errorText(err), true);
                });
              }
            },
            { label: "Cancel" }
          ]
        });
      }

      function play() {
        if (isRunning) {
          return;
        }
        paint(true);
        call("play").then(function () {
          // Follow the next poll for the real state.
        }, function (err) {
          paint(false);
          var errText = GT4.errorText(err);
          if (errText.indexOf("GSdx is not installed yet") !== -1) {
            promptGsdxInstall();
          } else {
            GT4.ui.toast(errText, true);
          }
          // Game files missing (the data folder was cleaned): back to Setup, where the player chooses the disc again.
          call("app_state").then(function (appState) {
            if (appState.setup && appState.setup.state !== "done") {
              GT4.go("#/setup");
            }
          }, function () {});
        });
      }

      function stop() {
        call("stop_game").then(function () {
          paint(false);
        }, function (err) {
          GT4.ui.toast(GT4.errorText(err), true);
        });
      }

      playButton.addEventListener("click", play);
      stopButton.addEventListener("click", stop);

      // Settings: a square button beside Play with the same look and focus (owner, 1.2.1).
      var gear = h("button", {
        class: "play play-square",
        type: "button",
        "aria-label": "Settings",
        title: "Settings",
        on: { click: function () { GT4.go("#/settings/graphics"); } }
      }, [gearIcon()]);

      // Slideshow: one img per slot (bg-slide-1..6 crossfade in CSS). The slots come from GT4.config.
      // At most six slots: css/app.css defines bg-slide-1..6.
      var backgrounds = ((GT4.config && GT4.config.backgrounds) || []).slice(0, 6);
      var slides = backgrounds.map(function (src, index) {
        return h("img", { class: "bg-slide bg-slide-" + (index + 1), src: src, alt: "" });
      });
      page.appendChild(h("div", { class: "art" }, slides.concat([h("div", { class: "shade" })])));
      var versionLabel = h("div", { class: "home-version", text: "" });
      page.appendChild(versionLabel);
      call("about_info").then(function (info) {
        versionLabel.textContent = "GT4Recomp " + (info.version || "");
      }, function () {});
      page.appendChild(h("div", { class: "home-bottom" }, [
        h("img", { class: "home-logo", src: "assets/logo-gt4-white.png", alt: "Gran Turismo 4" }),
        h("div", { class: "home-actions" }, [playButton, running, gear])
      ]));
      root.appendChild(page);
      document.body.classList.add("is-home");

      function poll() {
        call("app_state").then(function (appState) {
          paint(!!appState.game_running);
        }, function () {});
      }
      poll();
      timer = setInterval(poll, POLL_MS);

      // Dev harness only (dev/mock-data.js sets GT4.devFocus from ?focus=play): focus Play for screenshots.
      if (GT4.devFocus === "play") {
        playButton.focus({ preventScroll: true });
      }

      GT4.hooks.start = play;
      GT4.hooks.y = function () { GT4.go("#/settings/graphics"); };
      GT4.ui.setHints([
        { key: "A", label: "Play", run: play },
        { key: "Y", label: "Settings", run: function () { GT4.go("#/settings/graphics"); } }
      ]);

      return {
        destroy: function () {
          clearInterval(timer);
          document.body.classList.remove("is-home");
        }
      };
    }
  };
})(window);
