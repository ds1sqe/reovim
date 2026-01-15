# display/ - Display Driver

Frame buffer and window compositor.

## Source Location

`lib/drivers/display/src/`

## Key Traits

```rust
pub trait DisplayDriver: Send + Sync {
    fn size(&self) -> (u16, u16);
    fn resize(&mut self, width: u16, height: u16);

    fn draw(&mut self, x: u16, y: u16, cell: Cell);
    fn flush(&mut self) -> Result<()>;

    fn set_cursor(&mut self, x: u16, y: u16);
    fn set_cursor_style(&mut self, style: CursorStyle);
    fn show_cursor(&mut self, visible: bool);
}
```

## Cell and Style

```rust
pub struct Cell {
    pub char: char,
    pub style: Style,
}

pub struct Style {
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    pub modifiers: StyleModifiers,
}
```

## Cursor Styles

```rust
pub enum CursorStyle {
    Block,      // Normal mode (vim)
    Bar,        // Insert mode (vim) - thin vertical line
    Underline,  // Replace mode
    Hidden,
}

// Mode display behavior trait
pub trait ModeDisplay {
    fn cursor_style(&self) -> CursorStyle;  // Cursor shape for this mode
    fn status_text(&self) -> &'static str;   // Status line text (e.g., "NORMAL", "INSERT")
}
```

## Compositor

Layer-based rendering:

```rust
pub trait Compositor {
    fn push_layer(&mut self, layer: Box<dyn Layer>);
    fn pop_layer(&mut self);
    fn render(&mut self, display: &mut dyn DisplayDriver);
}

pub trait Layer {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn handle_event(&mut self, event: &Event) -> EventResult;
}
```

## Related Documents

- [Driver Overview](../overview.md) - Driver layer architecture
- [Input Driver](../input/overview.md) - Key/mouse events
