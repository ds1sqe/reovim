# Default Modules Bundle

This module aggregates all default policy handlers for reovim. It acts as a
"meta-module" that centralizes default behaviors, making it easy to swap out
all defaults at once for custom configurations.

## Included Policies

| Policy | Handler Module | Description |
|--------|----------------|-------------|
| Keybindings | `vim` module | Vim-like keybindings (normal, insert, visual, etc.) |
| Commands | `commands` module | Ex commands (`:w`, `:q`, `:wq`) |
| Operators | `operators` module | Vim operators (`d`, `y`, `c`) |
| Empty Session | `scratch-buffer` module | Create empty buffer on startup |

## Usage

The runner uses `defaults` to get all default handlers:

```rust
use reovim_module_defaults;

// Get empty session handler
let handler = reovim_module_defaults::empty_session_handler();
registry.register(Arc::new(handler));

// Get keybindings
let keybindings = reovim_module_defaults::keybindings();

// Get operators
let operators = reovim_module_defaults::operators();

// Get commands
let commands = reovim_module_defaults::commands();
```

## Architecture

```
runner -> modules/defaults -> modules/scratch-buffer
                           -> modules/vim
                           -> modules/operators
                           -> modules/commands
```

The runner is decoupled from specific policy modules. It only imports `defaults`,
which aggregates all the actual policy implementations.

## Adding New Default Policies

To add a new default policy:

1. Create the handler module in `modules/<name>/`
2. Add dependency to `modules/defaults/Cargo.toml`
3. Expose via appropriate method:
   - `keybindings()` for keybindings
   - `commands()` for commands
   - `operators()` for operators
   - `empty_session_handlers()` for session handlers (Module trait)
   - Add a factory function if needed (e.g., `empty_session_handler()`)
4. Update this README

## Future Policies (Planned)

These policies will be added when their systems are implemented:

- **Default theme** - When the theme system exists
- **Default statusline** - When the statusline module exists
- **Default editor options** - Currently in runner, may move here

## Module Trait Implementation

`DefaultsModule` implements the `Module` trait with:

- `empty_session_handlers()` - Returns `EmptySessionHandlerRegistration` descriptors
- `keybindings()` - Aggregates keybindings from vim module
- `dependencies()` - Declares sub-module dependencies

## Related Issues

- #369 - Empty session handler mechanism (initial implementation)
- #381 - Expand defaults module (this work)
- #265 - Dynamic module loading (will use wiring infrastructure)
