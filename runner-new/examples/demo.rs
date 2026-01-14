//! New Architecture Runner - Entry Point
//!
//! Demonstrates the complete wiring of:
//! - Kernel context
//! - Mode registry (Normal, Insert)
//! - Command registry (cursor movement, mode switching)
//! - Keymap registry (vim-style bindings)
//! - Event loop with fallback handler
//!
//! This demonstrates the "mechanism vs policy" separation:
//! - runner-new provides mechanism (registries, event loop)
//! - reovim-module-editor provides policy (modes, commands, fallback)

use std::sync::Arc;

use {
    reovim_driver_command::Command,
    reovim_driver_input::{KeyCode, KeyEvent},
    reovim_kernel::api::v1::KernelContext,
    reovim_module_editor::{
        EditorFallbackHandler, EditorMode,
        command::{
            CursorDown, CursorLeft, CursorRight, CursorUp, EnterInsertMode, EnterInsertModeAppend,
            ExitToNormal, all_commands,
        },
    },
    runner_new::{
        AppState, EventLoop,
        registry::{CommandRegistry, KeymapRegistry, ModeEntry, ModeRegistry},
    },
};

fn main() {
    println!("=== Reovim New Architecture Runner ===\n");

    // Phase 4 Demonstration: Wire everything together
    //
    // This shows how the kernel-driver-module architecture works:
    // 1. Kernel provides context (event bus, buffer manager, etc.)
    // 2. Drivers provide traits (Mode, ModeDisplay, CommandHandler)
    // 3. Modules provide implementations (EditorMode, cursor commands)
    // 4. Runner wires it all together

    // 1. Create kernel context
    let kernel = KernelContext::default();
    println!("✓ Created KernelContext");

    // 2. Create AppState with Normal mode as initial
    let app = AppState::new(kernel, EditorMode::NORMAL_ID);
    println!("✓ Created AppState (initial mode: Normal)");

    // 3. Create registries
    let mut mode_registry = ModeRegistry::new();
    let mut command_registry = CommandRegistry::new();
    let mut keymap_registry = KeymapRegistry::new();
    println!("✓ Created registries (Mode, Command, Keymap)");

    // 4. Register modes (EditorMode implements Mode, ModeDisplay, ModeInput)
    //    EditorMode can be cloned and used as all three traits
    let normal = Arc::new(EditorMode::Normal);
    let insert = Arc::new(EditorMode::Insert);

    mode_registry.register(
        ModeEntry::new(normal.clone())
            .with_display(normal.clone())
            .with_input(normal.clone()),
    );
    mode_registry.register(
        ModeEntry::new(insert.clone())
            .with_display(insert.clone())
            .with_input(insert),
    );
    println!("✓ Registered modes: Normal, Insert");

    // 5. Register commands
    for cmd in all_commands() {
        command_registry.register(Arc::from(cmd));
    }
    println!(
        "✓ Registered 7 commands: cursor-up/down/left/right, enter-insert, enter-insert-append, exit-to-normal"
    );

    // 6. Register keybindings
    //    Normal mode: vim-style navigation and mode switching
    keymap_registry.register_str(EditorMode::NORMAL_ID, "j", CursorDown.id());
    keymap_registry.register_str(EditorMode::NORMAL_ID, "k", CursorUp.id());
    keymap_registry.register_str(EditorMode::NORMAL_ID, "h", CursorLeft.id());
    keymap_registry.register_str(EditorMode::NORMAL_ID, "l", CursorRight.id());
    keymap_registry.register_str(EditorMode::NORMAL_ID, "i", EnterInsertMode.id());
    keymap_registry.register_str(EditorMode::NORMAL_ID, "a", EnterInsertModeAppend.id());

    //    Insert mode: escape to exit
    keymap_registry.register_str(EditorMode::INSERT_ID, "<Escape>", ExitToNormal.id());
    println!("✓ Registered keybindings:");
    println!("  Normal: j/k/h/l (cursor), i/a (enter insert)");
    println!("  Insert: <Escape> (exit to normal)");

    // 7. Create fallback handler (policy from editor module)
    let fallback = EditorFallbackHandler;
    println!("✓ Created EditorFallbackHandler");

    // 8. Create event loop
    let mut event_loop =
        EventLoop::new(app, mode_registry, command_registry, keymap_registry, fallback);
    println!("✓ Created EventLoop\n");

    // Demonstrate with test input
    println!("=== Demonstration ===\n");

    // Simulate a key sequence: j, i, Hello, <Escape>, k
    let test_keys = vec![
        ("j", KeyEvent::new(KeyCode::Char('j'))),
        ("i", KeyEvent::new(KeyCode::Char('i'))),
        ("H", KeyEvent::new(KeyCode::Char('H'))),
        ("e", KeyEvent::new(KeyCode::Char('e'))),
        ("l", KeyEvent::new(KeyCode::Char('l'))),
        ("l", KeyEvent::new(KeyCode::Char('l'))),
        ("o", KeyEvent::new(KeyCode::Char('o'))),
        ("<Escape>", KeyEvent::new(KeyCode::Escape)),
        ("k", KeyEvent::new(KeyCode::Char('k'))),
    ];

    let keys_iter: Vec<KeyEvent> = test_keys.iter().map(|(_, k)| *k).collect();
    let mut key_index = 0;

    event_loop = event_loop.with_key_reader(move || {
        if key_index < keys_iter.len() {
            let key = keys_iter[key_index];
            key_index += 1;
            Some(key)
        } else {
            None
        }
    });

    // Show initial state
    println!("Initial mode: {:?}", event_loop.app().current_mode());
    println!("\nProcessing keys: j, i, H, e, l, l, o, <Escape>, k\n");

    // Run the event loop
    if let Err(e) = event_loop.run() {
        eprintln!("Event loop error: {e}");
    }

    // Show final state
    println!("\nFinal mode: {:?}", event_loop.app().current_mode());
    if let Some(err) = event_loop.last_error() {
        println!("Last error: {err}");
    }

    println!("\n=== Architecture Verified ===");
    println!("✓ Mechanism (runner-new): Event loop, registries, key dispatch");
    println!("✓ Policy (editor module): Modes, commands, fallback handler");
    println!("✓ Clean separation: Runner has NO editor-specific code");
}
