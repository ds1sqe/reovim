# reo-cli: Special Key Input Problem

## Description
When sending special keys via `reo-cli keys`, vim-notation special keys like `<BS>`, `<Backspace>`, `<Esc>` are not being properly parsed/handled in certain contexts (e.g., microscope insert/interactor mode).

## Observed Behavior
```bash
# This sends 2 key events but backspace doesn't work in microscope interactor mode
cargo run -p reo-cli -- -t 127.0.0.1:17060 keys '<BS><BS>'
# Returns: {"injected": 2}
# But the input field still shows the original text

# This treats "Backspace" as literal characters
cargo run -p reo-cli -- -t 127.0.0.1:17060 keys 'Backspace'
# Adds "Backspace" as text to the input
```

## Expected Behavior
`<BS>` and `<Backspace>` should delete the character before the cursor in microscope input field.

## Possible Causes
1. Key parsing in reo-cli may not properly convert vim notation to crossterm `KeyEvent`
2. The `KeymapScope::SubMode(SubModeKind::Interactor(...))` may not have the Backspace binding active
3. The key event may be consumed by a different handler before reaching microscope

## Investigation Notes
- `<Esc>` works for mode switching (Insert -> Normal)
- `<Space>ff` works for opening files picker
- Regular character input works in microscope
- Navigation keys (j/k) work in microscope normal mode
- The backspace handler IS subscribed in the plugin code

## Files to Investigate
- `tools/reo-cli/src/keys.rs` - Key parsing
- `lib/core/src/input/key.rs` - Key event handling
- `plugins/features/microscope/src/lib.rs` - Keybindings (lines 742-791)

## Priority
Low - can use normal vim operations, this is mainly a testing convenience issue.
