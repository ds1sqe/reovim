//! Animation system for visual effects
//!
//! This module provides support for animated visual effects like:
//! - **Pulsing/Breathing**: Smooth color intensity cycling
//! - **Shimmer/Glow**: Subtle luminosity changes
//! - **Color Transitions**: Smooth transitions between colors
//!
//! # TODO: Configuration System
//!
//! Future work to add user-facing configuration:
//! - Enable/disable animations globally
//! - Configurable frame rate (default 30 fps)
//! - Effect intensity/duration multipliers
//! - Per-effect enable/disable (cursor pulse, mode transitions, etc.)
//! - Integration with settings menu plugin
//!
//! # Architecture
//!
//! The animation system follows the saturator pattern:
//! - `AnimationController` runs as a background tokio task
//! - Effects are registered via `AnimationCommand` messages
//! - Controller ticks at configurable frame rate (default 30 fps)
//! - Sends `RenderSignal` only when effects are active
//!
//! # Render Integration
//!
//! Use `AnimationRenderStage` to apply effects during rendering:
//! - Register the stage with `RenderStageRegistry`
//! - Stage queries `AnimationState` for active effects
//! - Effects are applied as highlights/decorations to `RenderData`
//!
//! # Quick Start
//!
//! ```ignore
//! use reovim_core::animation::AnimationSystem;
//!
//! // Initialize the animation system
//! let system = AnimationSystem::spawn(event_tx, 30);
//!
//! // Register the render stage
//! render_stage_registry.register(Arc::new(system.render_stage()));
//!
//! // Store the handle for plugin access
//! plugin_state.register(system.handle().clone());
//!
//! // Start an effect
//! system.handle().start(Effect::new(
//!     EffectId::new(0),
//!     EffectTarget::HighlightGroup(HighlightGroup::CursorLine),
//!     AnimatedStyle::pulse_bg(Color::Black, Color::DarkGrey, 2000),
//! ));
//! ```

mod color;
mod controller;
mod easing;
mod effect;
mod stage;

use std::sync::Arc;

use tokio::sync::{RwLock, mpsc};

use crate::event::InnerEvent;

pub use {
    color::AnimatedColor,
    controller::{AnimationCommand, AnimationHandle, AnimationState, spawn_animation_controller},
    easing::EasingFn,
    effect::{AnimatedStyle, Effect, EffectId, EffectTarget, SweepConfig, UiElementId},
    stage::AnimationRenderStage,
};

/// Bundled animation system components for easy integration
///
/// This struct holds all the components needed for the animation system:
/// - Shared state for render access
/// - Handle for starting/stopping effects
///
/// Use `spawn()` to create and start the animation controller.
pub struct AnimationSystem {
    /// Shared animation state
    state: Arc<RwLock<AnimationState>>,
    /// Handle to communicate with the controller
    handle: AnimationHandle,
}

impl AnimationSystem {
    /// Spawn the animation system
    ///
    /// This creates the shared state, spawns the background controller task,
    /// and returns a system instance with all components ready for integration.
    ///
    /// # Arguments
    /// * `event_tx` - Channel to send `RenderSignal` events to the runtime
    /// * `frame_rate` - Animation frame rate in fps (typically 30)
    #[must_use]
    pub fn spawn(event_tx: mpsc::Sender<InnerEvent>, frame_rate: u8) -> Self {
        let state = Arc::new(RwLock::new(AnimationState::default()));
        let handle = spawn_animation_controller(event_tx, frame_rate, Arc::clone(&state));

        Self { state, handle }
    }

    /// Get the animation handle for starting/stopping effects
    #[must_use]
    pub const fn handle(&self) -> &AnimationHandle {
        &self.handle
    }

    /// Get the shared state
    #[must_use]
    pub fn state(&self) -> Arc<RwLock<AnimationState>> {
        Arc::clone(&self.state)
    }

    /// Create a render stage that uses this system's state
    #[must_use]
    pub fn render_stage(&self) -> AnimationRenderStage {
        AnimationRenderStage::new(Arc::clone(&self.state))
    }
}
