/*
 * Achievements tab (RetroAchievements account login, session options, and Gran Turismo 4 achievement progress).
 * Commands:
 *   ra_status() -> {enabled, hardcore, encore, signedIn, username, displayName, score, scoreSoftcore, game}
 *   ra_login({username, password}) -> status
 *   ra_logout() -> status
 *   ra_set_options({enabled, hardcore, encore}) -> status
 */
(function (root) {
  "use strict";
  var GT4 = (root.GT4 = root.GT4 || {});
  var h = GT4.ui.h;

  function infoTip(title, text) {
    var tipId = "ra-tip-" + title.toLowerCase().replace(/[^a-z0-9]+/g, "-");
    var btn = h("button", {
      class: "info",
      type: "button",
      "aria-label": "About " + title,
      "aria-expanded": "false",
      "aria-describedby": tipId,
      on: {
        click: function (event) {
          event.stopPropagation();
          GT4.ui.toggleTip(btn);
        }
      }
    }, [
      "i",
      h("span", { class: "tip", id: tipId, role: "tooltip" }, [
        h("b", { text: title }),
        h("span", { text: text })
      ])
    ]);
    return btn;
  }

  function optionToggleRow(label, helpText, checked, onToggle) {
    var toggleBtn = h("button", {
      class: "toggle" + (checked ? " on" : ""),
      type: "button",
      role: "switch",
      "aria-checked": String(!!checked),
      "aria-label": label,
      data: { adjust: "1" },
      on: {
        click: function () {
          onToggle(!checked);
        }
      }
    });
    toggleBtn.addEventListener("gt4-adjust", function (event) {
      var next = event.detail.delta > 0;
      if (next !== !!checked) {
        onToggle(next);
      }
    });
    var labelSpan = h("span", { text: label });
    labelSpan.style.cursor = "pointer";
    labelSpan.addEventListener("click", function () {
      onToggle(!checked);
    });
    return {
      button: toggleBtn,
      row: h("div", { class: "row" }, [
        h("div", { class: "k" }, [
          labelSpan,
          infoTip(label, helpText)
        ]),
        h("div", { class: "ctl" }, [toggleBtn])
      ])
    };
  }

  GT4.tabPages.achievements = function (content, host) {
    var state = {
      enabled: true,
      hardcore: false,
      encore: false,
      signedIn: false,
      username: "",
      displayName: "",
      score: 0,
      scoreSoftcore: 0,
      game: null
    };

    var container = h("div", { class: "ra-page" });
    content.appendChild(container);
    // A later account/options action invalidates a slower status/login response.
    var requestSerial = 0;

    function applyState(next, toastMsg, requestId) {
      if (requestId !== requestSerial || !container.isConnected) {
        return;
      }
      if (next && typeof next === "object") {
        state.enabled = !!next.enabled;
        state.hardcore = !!next.hardcore;
        state.encore = !!next.encore;
        state.signedIn = !!next.signedIn;
        state.username = next.username || "";
        state.displayName = next.displayName || next.username || "";
        state.score = Number(next.score || 0);
        state.scoreSoftcore = Number(next.scoreSoftcore || 0);
        if (next.game !== undefined && next.game !== null) {
          state.game = next.game;
        } else if (!state.signedIn) {
          state.game = null;
        }
      }
      if (toastMsg) {
        GT4.ui.toast(toastMsg);
      }
      paint();
    }

    function saveOptions(enabled, hardcore, encore) {
      var requestId = ++requestSerial;
      GT4.backend.call("ra_set_options", {
        enabled: !!enabled,
        hardcore: !!hardcore,
        encore: !!encore
      }).then(function (res) {
        if (res && state.game && !res.game) {
          res.game = state.game;
        }
        applyState(res, "", requestId);
      }, function (err) {
        showError(err, requestId);
      });
    }

    function showError(err, requestId) {
      if (requestId === requestSerial && container.isConnected) {
        GT4.ui.toast(GT4.errorText(err), true);
      }
    }

    function openSignInModal() {
      var userInput = h("input", {
        class: "ra-input",
        type: "text",
        placeholder: "Username",
        autocomplete: "username",
        spellcheck: "false",
        value: state.username || ""
      });
      var passInput = h("input", {
        class: "ra-input",
        type: "password",
        placeholder: "Password",
        autocomplete: "current-password"
      });
      var statusLine = h("div", { class: "ra-login-error", text: "" });
      var busy = false;

      function submit(dialogApi) {
        if (busy) {
          return;
        }
        var u = (userInput.value || "").trim();
        var p = passInput.value || "";
        if (!u) {
          statusLine.textContent = "Enter your RetroAchievements username.";
          userInput.focus();
          return;
        }
        if (!p) {
          statusLine.textContent = "Enter your RetroAchievements password.";
          passInput.focus();
          return;
        }
        busy = true;
        var requestId = ++requestSerial;
        statusLine.textContent = "Signing in to retroachievements.org...";
        GT4.backend.call("ra_login", { username: u, password: p }).then(function (res) {
          busy = false;
          if (requestId !== requestSerial || !container.isConnected) {
            return;
          }
          dialogApi.close();
          applyState(res, "Signed in as " + (res.displayName || u), requestId);
        }, function (err) {
          busy = false;
          if (requestId === requestSerial && container.isConnected) {
            statusLine.textContent = GT4.errorText(err);
          }
        });
      }

      var dialog = GT4.ui.openDialog({
        title: "Sign in to RetroAchievements",
        text: "Your password is sent once over HTTPS to retroachievements.org to obtain an API token. Only your username and token are stored.",
        body: h("div", { class: "ra-form" }, [
          h("label", { class: "ra-field" }, [
            h("span", { class: "ra-field-label", text: "Username" }),
            userInput
          ]),
          h("label", { class: "ra-field" }, [
            h("span", { class: "ra-field-label", text: "Password" }),
            passInput
          ]),
          statusLine
        ]),
        buttons: [
          { label: "Cancel" },
          {
            label: "Sign in",
            primary: true,
            closes: false,
            run: function (api) {
              submit(api);
            }
          }
        ]
      });

      function onEnterKey(event) {
        if (event.key === "Enter") {
          event.preventDefault();
          submit(dialog);
        }
      }
      userInput.addEventListener("keydown", onEnterKey);
      passInput.addEventListener("keydown", onEnterKey);
      setTimeout(function () {
        userInput.focus();
      }, 20);
    }

    function confirmSignOut() {
      GT4.ui.openDialog({
        title: "Sign out of RetroAchievements?",
        text: "Your stored API token will be removed from this PC.",
        buttons: [
          { label: "Cancel" },
          {
            label: "Sign out",
            primary: true,
            danger: true,
            run: function () {
              var requestId = ++requestSerial;
              GT4.backend.call("ra_logout", {}).then(function (res) {
                applyState(res, "Signed out of RetroAchievements", requestId);
              }, function (err) {
                showError(err, requestId);
              });
            }
          }
        ]
      });
    }

    function refreshStatus() {
      var requestId = ++requestSerial;
      GT4.backend.call("ra_status", {}).then(function (res) {
        applyState(res, "Refreshed RetroAchievements status", requestId);
      }, function (err) {
        showError(err, requestId);
      });
    }

    function paint() {
      GT4.ui.clear(container);

      var actionBtn = state.signedIn
        ? h("button", { class: "btn danger", type: "button", on: { click: confirmSignOut } }, "Sign out")
        : h("button", { class: "btn primary", type: "button", on: { click: openSignInModal } }, "Sign in");

      container.appendChild(h("div", { class: "lib-head" }, [
        h("div", null, [
          h("h1", { text: "Achievements" }),
          h("p", {
            class: "note",
            text: "Sign in with your RetroAchievements account to unlock achievements in-game and record them on retroachievements.org."
          })
        ]),
        actionBtn
      ]));

      // Account summary section
      var accountSection = h("section", { class: "settings-section" }, [
        h("h2", { class: "section-head", text: "Account" })
      ]);

      if (state.signedIn) {
        accountSection.appendChild(h("div", { class: "row" }, [
          h("div", { class: "k" }, [h("span", { text: "Signed in as" })]),
          h("div", { class: "ctl wide" }, [
            h("span", { class: "val ra-user-badge", text: state.displayName || state.username })
          ])
        ]));
        accountSection.appendChild(h("div", { class: "row" }, [
          h("div", { class: "k" }, [h("span", { text: "Score" })]),
          h("div", { class: "ctl wide" }, [
            h("span", {
              class: "val",
              text: state.score + " pts Hardcore  ·  " + state.scoreSoftcore + " pts Softcore"
            })
          ])
        ]));
      } else {
        accountSection.appendChild(h("div", { class: "row" }, [
          h("div", { class: "k" }, [h("span", { text: "Status" })]),
          h("div", { class: "ctl wide" }, [
            h("span", { class: "val", text: "Not signed in" })
          ])
        ]));
      }
      container.appendChild(accountSection);

      // Session options section
      var optionsSection = h("section", { class: "settings-section" }, [
        h("h2", { class: "section-head", text: "Session Options" })
      ]);

      var enableRow = optionToggleRow(
        "Enable RetroAchievements",
        "Starts a RetroAchievements session when the game launches, evaluates PS2 memory triggers each frame, shows on-screen achievement popups, and submits unlocks to retroachievements.org.",
        state.enabled,
        function (val) {
          saveOptions(val, state.hardcore, state.encore);
        }
      );
      var hardcoreRow = optionToggleRow(
        "Hardcore mode",
        "Earns Hardcore points and mastery badges on RetroAchievements.",
        state.hardcore,
        function (val) {
          saveOptions(state.enabled, val, state.encore);
        }
      );
      var encoreRow = optionToggleRow(
        "Encore mode",
        "Allows achievements you have already unlocked to trigger again during this session.",
        state.encore,
        function (val) {
          saveOptions(state.enabled, state.hardcore, val);
        }
      );

      optionsSection.appendChild(enableRow.row);
      optionsSection.appendChild(hardcoreRow.row);
      optionsSection.appendChild(encoreRow.row);
      container.appendChild(optionsSection);

      // Gran Turismo 4 achievement progress section
      if (state.signedIn && state.game) {
        var game = state.game;
        var summaryText =
          "Unlocked " +
          game.unlocked_achievements +
          " of " +
          game.total_achievements +
          " achievements (" +
          game.unlocked_points +
          " / " +
          game.total_points +
          " pts)";

        var gameSection = h("section", { class: "settings-section" }, [
          h("h2", { class: "section-head", text: game.title || "Gran Turismo 4" }),
          h("div", { class: "row" }, [
            h("div", { class: "k" }, [h("span", { text: "Progress" })]),
            h("div", { class: "ctl wide" }, [h("span", { class: "val", text: summaryText })])
          ])
        ]);

        var listBox = h("div", { class: "ra-ach-list" });
        (game.achievements || []).forEach(function (ach) {
          var statusLabel = ach.unlocked_hardcore
            ? "Hardcore"
            : ach.unlocked
            ? "Unlocked"
            : "Locked";
          var statusClass = ach.unlocked_hardcore
            ? "ra-pill is-hc"
            : ach.unlocked
            ? "ra-pill is-unlocked"
            : "ra-pill is-locked";

          var titleRow = [h("span", { class: "ra-ach-title", text: ach.title })];
          if (ach.type_label) {
            titleRow.push(h("small", { class: "ra-ach-type", text: ach.type_label }));
          }

          listBox.appendChild(h("div", {
            class: "ra-ach-row" + (ach.unlocked ? " is-earned" : ""),
            tabindex: "0",
            data: { focusable: "1" }
          }, [
            h("div", { class: "ra-ach-main" }, [
              h("div", { class: "ra-ach-head" }, titleRow),
              h("div", { class: "ra-ach-desc", text: ach.description })
            ]),
            h("div", { class: "ra-ach-meta" }, [
              h("span", { class: "ra-ach-pts", text: ach.points + " pts" }),
              h("span", { class: statusClass, text: statusLabel })
            ])
          ]));
        });
        gameSection.appendChild(listBox);
        container.appendChild(gameSection);
      }

      host.entry(function () {
        return actionBtn;
      });
      host.setHooks({
        x: function () {
          if (state.signedIn) {
            refreshStatus();
          } else {
            openSignInModal();
          }
        }
      });
      host.hints([
        { key: "A", label: "Select" },
        {
          key: "X",
          label: state.signedIn ? "Refresh" : "Sign in",
          run: function () {
            if (GT4.hooks.x) {
              GT4.hooks.x();
            }
          }
        },
        { key: "B", label: "Back" }
      ]);
    }

    paint();
    var requestId = ++requestSerial;
    return GT4.backend.call("ra_status", {}).then(function (res) {
      applyState(res, "", requestId);
    }, function (err) {
      showError(err, requestId);
    });
  };
})(window);
