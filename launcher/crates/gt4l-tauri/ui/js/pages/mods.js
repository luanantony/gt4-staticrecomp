/*
 * Mods page controller (TASK-054).
 * Manages texture replacement packs and custom mods via Tauri backend commands:
 *   mods_list, mods_import, mods_set_enabled, mods_move, mods_remove, mods_details
 */
(function (root) {
  "use strict";
  var GT4 = (root.GT4 = root.GT4 || {});
  var h = GT4.ui.h;

  function mountMods(content, host) {
    var listContainer = h("div", { class: "mods-container" });
    content.appendChild(listContainer);

    var state = {
      items: [],
      renderer: "gsdx",
      selectedId: null,
      activeTextures: 0
    };

    function refresh() {
      GT4.backend.call("mods_list", {}).then(function (res) {
        state.items = (res && res.items) || [];
        state.activeTextures = (res && res.activeTextures) || 0;
        render();
      }).catch(function (err) {
        GT4.ui.toast(GT4.errorText(err), true);
        render();
      });
    }

    function toggleMod(id, currentEnabled) {
      GT4.backend.call("mods_set_enabled", { id: id, enabled: !currentEnabled }).then(function () {
        refresh();
      }).catch(function (err) {
        GT4.ui.toast(GT4.errorText(err), true);
      });
    }

    function moveMod(id, delta) {
      var item = state.items.find(function (m) { return m.id === id; });
      if (!item) return;
      var newPriority = Math.max(0, item.priority + delta);
      GT4.backend.call("mods_move", { id: id, newPriority: newPriority }).then(function () {
        refresh();
      }).catch(function (err) {
        GT4.ui.toast(GT4.errorText(err), true);
      });
    }

    function removeMod(id, name) {
      GT4.ui.openDialog({
        title: "Remove texture pack?",
        text: "Are you sure you want to uninstall \"" + (name || id) + "\"? The mod folder will be moved to the Recycle Bin.",
        buttons: [
          { label: "Cancel" },
          {
            label: "Delete",
            primary: true,
            danger: true,
            run: function () {
              GT4.backend.call("mods_remove", { id: id }).then(function () {
                GT4.ui.toast("Mod removed.");
                refresh();
              }).catch(function (err) {
                GT4.ui.toast(GT4.errorText(err), true);
              });
            }
          }
        ]
      });
    }

    function showDetails(id) {
      GT4.backend.call("mods_details", { id: id }).then(function (details) {
        if (!details) return;
        GT4.ui.openDialog({
          title: details.name || details.id,
          text: (details.description || "No description provided.") + "\n\n" +
                "• Version: " + (details.version || "1.0.0") + "\n" +
                "• Author: " + (details.author || "Unknown") + "\n" +
                "• Textures: " + (details.textureCount || 0) + " files\n" +
                "• Target Serial: " + (details.game || "SCUS-97328") + "\n" +
                "• Size: " + formatBytes(details.sizeBytes || 0),
          buttons: [
            { label: "Close", primary: true }
          ]
        });
      }).catch(function (err) {
        GT4.ui.toast(GT4.errorText(err), true);
      });
    }

    function importFile() {
      GT4.backend.call("mods_import", {}).then(function (report) {
        if (report) {
          GT4.ui.toast("Installed \"" + (report.name || report.id) + "\" (" + (report.textureCount || 0) + " textures).");
          refresh();
        }
      }).catch(function (err) {
        GT4.ui.toast(GT4.errorText(err), true);
      });
    }

    function openInbox() {
      GT4.backend.call("open_inbox_folder", {}).catch(function (err) {
        GT4.ui.toast(GT4.errorText(err), true);
      });
    }

    function formatBytes(bytes) {
      if (!bytes || bytes <= 0) return "0 B";
      var mb = bytes / (1024 * 1024);
      if (mb >= 1024) {
        return (mb / 1024).toFixed(2) + " GB";
      }
      return mb.toFixed(1) + " MB";
    }

    function render() {
      GT4.ui.clear(listContainer);

      // Header
      var header = h("div", { class: "mods-header", style: "display: flex; justify-content: space-between; align-items: flex-end; margin-bottom: 24px; padding-bottom: 16px; border-bottom: 1px solid var(--hair);" }, [
        h("div", {}, [
          h("h1", { text: "Mods & Texture Packs", style: "font: 300 28px var(--text); color: var(--white); margin-bottom: 4px;" }),
          h("p", { text: "Custom textures for PCSX2 GS. Top packs take priority in case of duplicate textures.", style: "color: var(--dim); font-size: 13px;" })
        ]),
        h("div", { class: "mods-actions", style: "display: flex; gap: 10px;" }, [
          h("button", { class: "btn", type: "button", text: "Install .zip...", on: { click: importFile } }),
          h("button", { class: "btn secondary", type: "button", text: "Open Inbox Folder", on: { click: openInbox } })
        ])
      ]);
      listContainer.appendChild(header);

      if (state.items.length === 0) {
        var empty = h("div", { class: "mods-empty", style: "text-align: center; padding: 48px 24px; border: 2px dashed rgba(255,255,255,0.12); border-radius: 6px; background: rgba(16,18,22,0.3);" }, [
          h("h3", { text: "No mods or texture packs installed", style: "font: 300 20px var(--text); color: var(--white); margin-bottom: 8px;" }),
          h("p", { text: "Drop a texture pack zip file here, or place it into the Inbox folder to install.", style: "color: var(--dim); font-size: 13px; margin-bottom: 20px;" }),
          h("button", { class: "btn", type: "button", text: "Choose .zip Archive...", on: { click: importFile } })
        ]);
        listContainer.appendChild(empty);
        return;
      }

      var list = h("div", { class: "mod-list" });
      state.items.forEach(function (item, idx) {
        var row = h("div", {
          class: "mod-row" + (item.enabled ? "" : " is-disabled"),
          style: "display: flex; align-items: center; padding: 12px 16px; background: rgba(22,24,28,0.45); border: 1px solid var(--hair); border-radius: 4px; margin-bottom: 8px; cursor: pointer;",
          dataset: { itemId: item.id },
          on: {
            click: function () { showDetails(item.id); }
          }
        }, [
          h("span", { text: "#" + (idx + 1), style: "width: 40px; font-weight: 600; font-family: monospace; color: var(--dim);" }),
          h("div", { style: "width: 36px;", on: { click: function (e) { e.stopPropagation(); } } }, [
            h("input", {
              type: "checkbox",
              checked: item.enabled,
              on: {
                change: function () { toggleMod(item.id, item.enabled); }
              }
            })
          ]),
          h("div", { style: "flex: 1; min-width: 0;" }, [
            h("div", { text: item.name || item.id, style: "font-weight: 500; font-size: 14px; color: var(--white); margin-bottom: 2px;" }),
            h("div", { text: "v" + (item.version || "1.0.0") + (item.author ? " • by " + item.author : ""), style: "font-size: 12px; color: var(--dim);" })
          ]),
          h("div", { style: "text-align: right; padding-right: 16px;" }, [
            h("div", { text: formatBytes(item.sizeBytes), style: "font-size: 13px; color: var(--silver);" }),
            h("div", { text: (item.textureCount || 0) + " textures", style: "font-size: 11px; color: var(--dim);" })
          ]),
          h("div", { style: "display: flex; gap: 6px;", on: { click: function (e) { e.stopPropagation(); } } }, [
            h("button", {
              class: "iconbtn",
              title: "Move Up",
              text: "▲",
              disabled: idx === 0,
              on: { click: function () { moveMod(item.id, -1); } }
            }),
            h("button", {
              class: "iconbtn",
              title: "Move Down",
              text: "▼",
              disabled: idx === state.items.length - 1,
              on: { click: function () { moveMod(item.id, 1); } }
            }),
            h("button", {
              class: "iconbtn",
              title: "Details",
              text: "ℹ",
              on: { click: function () { showDetails(item.id); } }
            }),
            h("button", {
              class: "iconbtn del",
              title: "Remove",
              text: "✕",
              on: { click: function () { removeMod(item.id, item.name); } }
            })
          ])
        ]);
        list.appendChild(row);
      });

      listContainer.appendChild(list);
    }

    // Input hooks for controller and keyboard
    GT4.hooks = {
      x: function () {
        var active = document.activeElement;
        var id = active && active.dataset && active.dataset.itemId;
        if (id) {
          var item = state.items.find(function (m) { return m.id === id; });
          removeMod(id, item && item.name);
        }
      },
      y: function () {
        var active = document.activeElement;
        var id = active && active.dataset && active.dataset.itemId;
        if (id) {
          showDetails(id);
        }
      },
      start: function () {
        importFile();
      }
    };

    GT4.ui.setHints([
      { key: "D-Pad", label: "Navigate" },
      { key: "A", label: "Toggle" },
      { key: "Y", label: "Details" },
      { key: "X", label: "Remove" },
      { key: "Start", label: "Import" },
      { key: "B", label: "Back" }
    ]);

    refresh();

    return {
      destroy: function () {
        GT4.hooks = {};
      }
    };
  }

  // Register on tabPages for Settings integration and top-level pages
  GT4.tabPages = GT4.tabPages || {};
  GT4.tabPages.mods = mountMods;

  GT4.pages = GT4.pages || {};
  GT4.pages.mods = { mount: mountMods };

})(window);
