# Controller Button Prompts Replacement Packs (TASK-165)

This directory hosts optional button prompt texture packs for Xbox and Nintendo Switch controller layouts.
These assets are loaded via the hardware GS texture cache replacement system (`GT4_HW_TEXTURES` in CoreGS 2.0 and `[Folders] Textures` in GSdx).

## Directories
* `xbox/`: Prompts mapping DualShock 2 buttons to standard Xbox glyphs (A, B, X, Y, LB, RB, LT, RT, Menu, View).
* `switch/`: Prompts mapping DualShock 2 buttons to Nintendo Switch glyphs (B, A, Y, X, L, R, ZL, ZR, +, -).

## Format Specification
Replacements must follow Format P / PCSX2 replacement hash naming:
`<textureHash>[-<clutHash>][-r<W>x<H>]-<bits>.png`

Refer to `docs/BUTTON-PROMPTS-SETUP.md` for architecture details.
