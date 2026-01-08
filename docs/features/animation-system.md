# Animation System

Reovim's animation system provides visual feedback through smooth animated effects. This document covers the architecture, effect types, and integration points.

## Architecture Overview

```
AnimationController
       │
       ├── Effect Registry
       │   ├── Effect 1 (Pulse)
       │   ├── Effect 2 (Fade)
       │   └── Effect N
       │
       ├── Tick Loop (30 fps default)
       │
       └── Render Stage Integration
```

## Core Components

### AnimationController

The central coordinator for all animations:

```rust
pub struct AnimationController {
    effects: HashMap<AnimationId, ActiveEffect>,
    frame_rate: u32,            // Default: 30 fps
    tick_handle: Option<JoinHandle<()>>,
}
```

### ActiveEffect

Represents a running animation:

```rust
pub struct ActiveEffect {
    pub effect: Box<dyn Effect>,
    pub target: EffectTarget,
    pub started_at: Instant,
    pub duration: Duration,
    pub mode: AnimationMode,
}
```

### EffectTarget

Specifies what the effect applies to:

```rust
pub enum EffectTarget {
    // Single cell
    Cell { row: usize, col: usize },

    // Range of text
    Range { start: Position, end: Position },

    // Entire line(s)
    Lines { start: usize, end: usize },

    // Full screen
    Screen,
}
```

## Effect Types

### Pulse

Alternating intensity animation:

```rust
pub struct PulseEffect {
    pub color: Color,
    pub intensity_range: (f32, f32),  // Min/max intensity
    pub cycles: u32,                   // Number of pulses
}
```

Used for: Paste feedback, search highlights.

### Fade

Gradual opacity transition:

```rust
pub struct FadeEffect {
    pub from_alpha: f32,
    pub to_alpha: f32,
    pub easing: EasingFunction,
}
```

Used for: Yank highlights, cursor trail.

### Shimmer

Wave-like color sweep:

```rust
pub struct ShimmerEffect {
    pub color: Color,
    pub wave_width: usize,
    pub direction: Direction,  // Left, Right, Up, Down
}
```

Used for: Loading indicators.

### Glow

Expanding/contracting halo:

```rust
pub struct GlowEffect {
    pub color: Color,
    pub radius_range: (f32, f32),
    pub intensity: f32,
}
```

Used for: Focus indicators, notifications.

## Animation Modes

```rust
pub enum AnimationMode {
    Once,        // Play once and remove
    Loop,        // Repeat indefinitely
    PingPong,    // Alternate forward/backward
}
```

## Creating Animations

### Using AnimationHandle

```rust
// Get handle from runtime
let handle = runtime.animation_handle();

// Start a pulse animation
handle.start(AnimationConfig {
    effect: Box::new(PulseEffect {
        color: Color::Cyan,
        intensity_range: (0.3, 1.0),
        cycles: 2,
    }),
    target: EffectTarget::Range { start, end },
    duration: Duration::from_millis(600),
    mode: AnimationMode::Once,
});
```

### Animation IDs

Animations can be named for later control:

```rust
// Start with ID
handle.start_with_id("cursor_pulse", config);

// Stop by ID
handle.stop("cursor_pulse");

// Check if running
if handle.is_running("cursor_pulse") {
    // ...
}
```

## Built-in Animations

### Yank Animation

Gold fade effect when yanking text:

```rust
// Triggered automatically on yank
AnimationConfig {
    effect: Box::new(FadeEffect {
        from_alpha: 0.8,
        to_alpha: 0.0,
        easing: EasingFunction::EaseOut,
    }),
    target: EffectTarget::Range { yanked_range },
    duration: Duration::from_millis(400),
    mode: AnimationMode::Once,
}
```

### Paste Animation

Cyan pulse when pasting:

```rust
AnimationConfig {
    effect: Box::new(PulseEffect {
        color: Color::Cyan,
        intensity_range: (0.4, 1.0),
        cycles: 2,
    }),
    target: EffectTarget::Range { pasted_range },
    duration: Duration::from_millis(600),
    mode: AnimationMode::Once,
}
```

### Landing Page Animation

Breathing effect for ASCII art:

```rust
AnimationConfig {
    effect: Box::new(PulseEffect {
        color: Color::White,
        intensity_range: (0.6, 1.0),
        cycles: 0,  // Infinite
    }),
    target: EffectTarget::Lines { ascii_art_lines },
    duration: Duration::from_millis(2000),
    mode: AnimationMode::PingPong,
}
```

## Easing Functions

```rust
pub enum EasingFunction {
    Linear,
    EaseIn,       // Slow start
    EaseOut,      // Slow end
    EaseInOut,    // Slow start and end
    Bounce,       // Bounce effect
    Elastic,      // Spring effect
}
```

Easing functions control animation progression:

```rust
// Linear: constant speed
fn linear(t: f32) -> f32 { t }

// EaseOut: fast start, slow end
fn ease_out(t: f32) -> f32 { 1.0 - (1.0 - t).powi(2) }

// EaseInOut: smooth start and end
fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}
```

## Render Integration

Animations integrate with the render pipeline via `AnimationRenderStage`:

```rust
impl RenderStage for AnimationRenderStage {
    fn name(&self) -> &'static str { "animation" }
    fn priority(&self) -> u32 { 250 }  // After visual selection

    fn execute(&self, data: &mut RenderData, ctx: &RenderContext) {
        for effect in self.controller.active_effects() {
            effect.apply(data);
        }
    }
}
```

## Frame Rate and Performance

### Configurable Frame Rate

```rust
// Default: 30 fps
controller.set_frame_rate(30);

// Higher for smoother animations (more CPU)
controller.set_frame_rate(60);

// Lower for reduced CPU usage
controller.set_frame_rate(15);
```

### Performance Considerations

- Effects are batched per frame
- Only affected regions trigger re-render
- Frame skipping when system is under load
- Animations pause when window is unfocused (future)

### CPU Impact

| Frame Rate | CPU Usage | Smoothness |
|------------|-----------|------------|
| 15 fps | Low | Choppy |
| 30 fps | Medium | Smooth |
| 60 fps | Higher | Very smooth |

## Plugin Integration

Plugins can create custom animations:

```rust
impl Plugin for MyPlugin {
    fn subscribe(&self, bus: &EventBus, state: Arc<PluginStateRegistry>) {
        let handle = state.get::<AnimationHandle>();

        bus.subscribe::<MyEvent>(move |event| {
            handle.start(AnimationConfig {
                effect: Box::new(CustomEffect::new()),
                target: EffectTarget::Cell { row, col },
                duration: Duration::from_millis(300),
                mode: AnimationMode::Once,
            });
        });
    }
}
```

### Custom Effects

Implement the `Effect` trait:

```rust
pub trait Effect: Send + Sync {
    fn name(&self) -> &'static str;

    fn compute_style(&self, progress: f32, base_style: Style) -> Style;

    fn is_complete(&self, elapsed: Duration, duration: Duration) -> bool {
        elapsed >= duration
    }
}

struct MyCustomEffect {
    color: Color,
}

impl Effect for MyCustomEffect {
    fn name(&self) -> &'static str { "custom" }

    fn compute_style(&self, progress: f32, base: Style) -> Style {
        let alpha = 1.0 - progress;
        base.with_fg(self.color.with_alpha(alpha))
    }
}
```

## Source Files

- `lib/core/src/animation/mod.rs` - Core types and exports
- `lib/core/src/animation/controller.rs` - AnimationController
- `lib/core/src/animation/effect.rs` - Effect trait and built-in effects
- `lib/core/src/animation/render_stage.rs` - Render pipeline integration
- `lib/core/src/animation/easing.rs` - Easing functions
