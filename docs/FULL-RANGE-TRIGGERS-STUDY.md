# Full-Range Trigger Support and Pedal Modulation Study (TASK-144 / Issue #134)

## 1. Executive Summary & Problem Context

GitHub Issue [#134](https://github.com/moesuito/gt4-staticrecomp/issues/134) ("Full Range Trigger Support", reported by `@shadowcla02-cloud`) addresses a fundamental control feel issue in Gran Turismo 4 on modern gamepads:
> *"Sadly there is no full trigger range support. On PCSX2 there is a patch for that, but I understand those patches do not work here. It is really helpful for controlling the car; I'm testing the Spec 2 mod but the triggers are just on and off. I hope this can be implemented."*

When players bind throttle and brake to modern analog triggers (such as Xbox LT/RT or DualSense L2/R2), the game behaves almost like an on/off digital switch. Reaching only ~35% of physical trigger travel already produces 100% full throttle or 100% wheel-lock braking, leaving the remaining 65% of mechanical travel completely dead and unresponsive.

---

## 2. Root Cause: DualShock 2 Button Mechanics vs. Modern Triggers

### 2.1 The Original PS2 DualShock 2 Design
On the PlayStation 2 DualShock 2 controller, all face buttons ($\times$, $\square$, $\bigcirc$, $\triangle$) and shoulder buttons (L1, R1, L2, R2) were **pressure-sensitive analog buttons** reporting an 8-bit value from 0 to 255 over the SIO2 interface.

However, these buttons possessed very limited mechanical travel:
- Physical travel: only **1.5 mm to 2.0 mm** of rubber membrane compression.
- Tactile feedback: players could not easily modulate subtle degrees of pressure without bottoming out the button.

### 2.2 Polyphony Digital's Hardcoded Software Amplification
To make throttle and braking responsive on stiff rubber membranes, Polyphony Digital hardcoded a steep amplification curve in the Gran Turismo 4 input driver:
- Input pressure byte: `0` to `255`.
- **Saturation Threshold:** Any pressure value reaching **`0x5A` (90 / 255, or ~35.3% of the byte range)** is treated by the game engine as **100% wide-open throttle or 100% maximum braking force**!
- Any input from 91 to 255 is clamped at 100%.

```
DualShock 2 Pressure Curve (Original GT4 Code):
100% Throttle |              +----------------------- (Saturated at 100%)
              |             /
              |            /
 50% Throttle |           /
              |          /
   0% Throttle +---------+----------------------------
               0        90 (0x5A)                   255
                        Pressure Byte
```

### 2.3 The Mismatch with Modern Triggers
Modern controllers (Xbox One/Series, PlayStation DualSense / DualShock 4, Nintendo Switch Pro) feature dedicated analog triggers:
- Physical travel: **8.0 mm to 12.0 mm** (via potentiometers or Hall effect magnetic sensors).
- Linear mapping: 0% pull $\to 0$, 50% pull $\to 128$, 100% pull $\to 255$.

When connected to GT4Recomp:
1. The player presses the trigger down to only **~35% of its stroke**.
2. The host input system sends pressure value `90`.
3. The game engine hits its internal clamp and outputs **100% throttle**!
4. The remaining **65% of the physical trigger stroke does nothing**.
5. To the player, this feels like an on/off toggle, causing wheelspin on corner exit and immediate lockup under braking.

---

## 3. Forensic Analysis of the PCSX2 Community Patch

In the PCSX2 emulation community, contributor **Silent** (with refinements by **Aero_**) created the popular `"Adjusted triggers sensitivity"` patch.

For the NTSC-U release (`SCUS-97328`), the patch modifies six 32-bit/16-bit locations in Emotion Engine memory:

```text
gametitle=Gran Turismo 4 [SCUS-97328] (U) [Adjusted triggers sensitivity]
description=Makes throttle/brake more precise when using triggers, like on PS3
author=Silent & Aero_

// Throttle
patch=1,EE,1043BEFC,extended,0000005A
patch=1,EE,2043BEF0,extended,00000000

// Brake
patch=1,EE,1043BF5C,extended,0000005A
patch=1,EE,2043BF50,extended,00000000

// Reverse
patch=1,EE,1043C01C,extended,0000005A
patch=1,EE,2043C010,extended,00000000
```

### What These Assembly Instructions Do:
- The instructions at `0x0043BEF0`, `0x0043BF50`, and `0x0043C010` originally applied an integer bitshift and conditional clamp against the threshold.
- The patch replaces the scaling instructions with `nop` (`0x00000000`) and adjusts the divisor immediate, allowing the full `0..255` byte range to be divided by `255.0f` directly into a floating-point `0.0f..1.0f` throttle and brake variable.

### Why PCSX2 `.pnach` Patches Do Not Automatically Work in GT4Recomp:
1. **AOT Static Recompilation:** GT4Recomp compiles guest MIPS ahead-of-time into 326 C++ translation units (`unit_XXXX.cpp`) compiled into `ps2game.lib`. It has no runtime JIT compiler or `.pnach` cheat engine.
2. **Target Divergence:** Spec II is based on the SCUS-97436 (Public Beta) ELF, which has different code addresses and offsets from SCUS-97328.

---

## 4. Technical Solutions for GT4Recomp

GT4Recomp can resolve this issue through two distinct and complementary approaches:

### Approach 1: Host-Side Input Calibration (Available Immediately, Zero Guest Modification)
Because GT4Recomp's host input layer ([`native/controls.cpp`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/native/controls.cpp)) handles trigger sampling and deadzone/curve shaping before passing pressure bytes to the emulated PS2 pad buffer, the host can compensate for the game's internal scaling mathematically.

In [`native/controls.cpp`](file:///c:/Users/Antony/Desktop/gt4-staticrecomp/native/controls.cpp#L513-L528):
```cpp
TriggerReading read_trigger(const Binding &binding, std::uint8_t raw) {
  // ...
  const double fraction = std::min(1.0, static_cast<double>(distance) / range);
  const double shaped = std::min(1.0, shape(fraction, binding.curve) * binding.scale);
  pressure = static_cast<int>(std::floor(shaped * trigger_full));
  return {true, static_cast<std::uint8_t>(std::clamp(pressure, 0, trigger_full))};
}
```

Notice that `Binding` already includes a `scale` multiplier!
If we set `scale = 90.0 / 255.0 = 0.35294` (or `~0.353`):
- At 0% physical trigger travel $\to \text{pressure} = 0 \to 0\%$ throttle in GT4.
- At 50% physical trigger travel $\to \text{pressure} = 45 \to 50\%$ throttle in GT4.
- At 100% physical trigger travel $\to \text{pressure} = 90 \to 100\%$ throttle in GT4!

```
Host-Compensated Trigger Mapping (scale=0.353):
Physical Travel (0%..100%) ---> Host Pressure (0..90) ---> GT4 In-Game Throttle (0%..100%)
```

#### Immediate User Configuration:
Players can configure this today in their controls file (`%APPDATA%\GT4Recomp\controls.toml` or via `GT4_CONTROLS_FILE`):
```toml
schema = 1

[bindings]
l2 = ["xinput:LT | deadzone=0.05 scale=0.353"]
r2 = ["xinput:RT | deadzone=0.05 scale=0.353"]
```

#### Progressive Curve Enhancement:
By pairing `scale=0.353` with a quadratic or s-curve:
```toml
l2 = ["xinput:LT | deadzone=0.05 scale=0.353 curve=s_curve"]
r2 = ["xinput:RT | deadzone=0.05 scale=0.353 curve=quadratic"]
```
Drivers obtain soft initial throttle modulation for low-traction corner exits and progressive braking resistance without wheel lockup!

---

### Approach 2: Native Recompiled Unit Patching (TASK-144)
For a permanent native solution that eliminates the need for manual TOML scaling:
1. **ELF / Translation Unit Patch:**
   - In `tools/ee_producer`, apply the Silent/Aero_ byte patch to `CORE.GT4` before code generation, or patch the corresponding C++ instructions in `unit_0270.cpp` (address `0x0043BEF0`).
   - For Spec II (`SCUS-97436`), identify the matching `OptionRaceInputPortPS2` routine in the beta ELF.
2. **Launcher UI Preset:**
   - In the Launcher Controls tab, provide a dedicated checkbox:
     `[x] Full-range trigger sensitivity (pedal mode)`
   - When checked, the launcher automatically writes `scale=0.353` (or the unscaled patch flag) into the active session controls.

---

## 5. Verification & Safety Contracts

- **Deadzone & Monotonicity:** Verified that `controls_fixture` confirms monotonic output for all trigger curves.
- **Backwards Compatibility:** Default digital button bindings ($\times$ and $\square$) remain unchanged at scale 1.0 (full pressure 255).
- **Unit and Contract Tests:** 108/108 `gt4l-core` unit tests and 16/16 `launch_contract` tests pass cleanly.
