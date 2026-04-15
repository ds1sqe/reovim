# reovim-driver-tui

Terminal User Interface driver for Reovim. Provides the **mechanism** for terminal-based rendering and input handling.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  TUI Client (lib/clients/tui/)                   POLICY     │
│    - Layout decisions                                       │
│    - Styling and theming                                    │
│    - What content to display                                │
├─────────────────────────────────────────────────────────────┤
│  TUI Driver (this crate)                         MECHANISM  │
│    - Terminal session (raw mode, alternate screen)          │
│    - Input event stream with vim notation                   │
│    - Screen rendering primitives                            │
│    - Cursor management                                      │
├─────────────────────────────────────────────────────────────┤
│  Display Driver (lib/drivers/display/)           MECHANISM  │
│    - FrameBuffer, Style, FrameRenderer                      │
└─────────────────────────────────────────────────────────────┘
```

## Components

### Terminal

RAII session guard that manages terminal state:
- Enters raw mode and alternate screen on creation
- Automatically restores on drop

```rust
let mut terminal = Terminal::enter()?;
// Terminal in raw mode...
// Restored automatically when `terminal` drops
```

### InputReader

Async input event stream with vim key notation:

```rust
let mut input = InputReader::new();
while let Some(event) = input.next_event().await {
    match event {
        PlatformEvent::Key(key) => {
            // key.vim_notation is "<C-w>", "j", "<Esc>", etc.
            send_to_server(&key.vim_notation);
        }
        PlatformEvent::Resize(r) => handle_resize(r.width, r.height),
        PlatformEvent::Mouse(m) => handle_mouse(m),
        _ => {}
    }
}
```

### Screen

Frame buffer with integrated rendering:

```rust
let mut screen = Screen::new(80, 24);

// Drawing primitives
screen.write_str(0, 0, "Hello", &Style::default());
screen.put_char(10, 5, '█', &style);
screen.fill_horizontal(0, 23, 80, ' ', &status_style);
screen.draw_box(5, 5, 20, 10, &border_style);

// Render to terminal
screen.render(&mut terminal)?;
```

### Cursor

Cursor state management with dirty-flag optimization:

```rust
let mut cursor = Cursor::new();
cursor.move_to(10, 5);
cursor.set_style(CursorStyle::BlinkingBar);
cursor.show();
cursor.apply()?; // Only sends terminal commands if state changed
```

## Vim Key Notation

The `InputReader` translates crossterm key events to vim notation:

| Key | Notation |
|-----|----------|
| `a` | `a` |
| `Ctrl+a` | `<C-a>` |
| `Alt+a` | `<A-a>` |
| `Shift+a` | `A` |
| `Escape` | `<Esc>` |
| `Enter` | `<CR>` |
| `Backspace` | `<BS>` |
| `F1` | `<F1>` |
| `Arrow Left` | `<Left>` |

## License

AGPL-3.0-only
