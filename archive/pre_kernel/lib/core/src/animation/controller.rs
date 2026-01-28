//! Animation controller - background task for managing effects
//!
//! The controller follows the saturator pattern:
//! - Runs as a background tokio task
//! - Receives commands via mpsc channel
//! - Ticks effects at configurable frame rate
//! - Sends `RenderSignal` when effects are active

use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::sync::{RwLock, mpsc};

use crate::event::RuntimeEvent;

use super::effect::{Effect, EffectId, EffectTarget};

/// Commands sent to the animation controller
#[derive(Debug)]
pub enum AnimationCommand {
    /// Start a new effect
    Start(Effect),
    /// Stop an effect by ID
    Stop(EffectId),
    /// Stop all effects for a target
    StopTarget(EffectTarget),
    /// Clear all effects
    ClearAll,
    /// Set frame rate (fps)
    SetFrameRate(u8),
}

/// Handle to communicate with the animation controller
#[derive(Debug, Clone)]
pub struct AnimationHandle {
    /// Send commands to the controller
    tx: mpsc::Sender<AnimationCommand>,
    /// Next effect ID (shared counter)
    next_id: Arc<std::sync::atomic::AtomicU64>,
}

impl AnimationHandle {
    /// Start a new effect, returning its ID
    #[must_use]
    pub fn start(&self, mut effect: Effect) -> Option<EffectId> {
        let id = EffectId::new(
            self.next_id
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        );
        effect.id = id;
        self.tx.try_send(AnimationCommand::Start(effect)).ok()?;
        Some(id)
    }

    /// Stop an effect by ID
    pub fn stop(&self, id: EffectId) {
        let _ = self.tx.try_send(AnimationCommand::Stop(id));
    }

    /// Stop all effects for a target
    pub fn stop_target(&self, target: EffectTarget) {
        let _ = self.tx.try_send(AnimationCommand::StopTarget(target));
    }

    /// Clear all effects
    pub fn clear_all(&self) {
        let _ = self.tx.try_send(AnimationCommand::ClearAll);
    }

    /// Set the animation frame rate
    pub fn set_frame_rate(&self, fps: u8) {
        let _ = self.tx.try_send(AnimationCommand::SetFrameRate(fps));
    }
}

/// Shared state for accessing current effects from render
#[derive(Debug, Default)]
pub struct AnimationState {
    /// Active effects by target
    pub(crate) effects: HashMap<EffectTarget, Vec<Effect>>,
}

impl AnimationState {
    /// Get effects for a specific target
    #[must_use]
    pub fn get_effects(&self, target: &EffectTarget) -> Option<&[Effect]> {
        self.effects.get(target).map(Vec::as_slice)
    }

    /// Check if there are any active effects
    #[must_use]
    pub fn has_active_effects(&self) -> bool {
        !self.effects.is_empty()
    }

    /// Get all active effects
    pub fn all_effects(&self) -> impl Iterator<Item = &Effect> {
        self.effects.values().flatten()
    }
}

/// Internal controller state
struct AnimationController {
    /// Active effects by target
    effects: HashMap<EffectTarget, Vec<Effect>>,
    /// Frame rate in fps
    frame_rate: u8,
}

impl AnimationController {
    fn new(frame_rate: u8) -> Self {
        Self {
            effects: HashMap::new(),
            frame_rate,
        }
    }

    fn has_active_effects(&self) -> bool {
        !self.effects.is_empty()
    }

    fn handle_command(&mut self, cmd: AnimationCommand) {
        match cmd {
            AnimationCommand::Start(effect) => {
                self.effects
                    .entry(effect.target.clone())
                    .or_default()
                    .push(effect);
            }
            AnimationCommand::Stop(id) => {
                for effects in self.effects.values_mut() {
                    effects.retain(|e| e.id != id);
                }
                // Remove empty entries
                self.effects.retain(|_, v| !v.is_empty());
            }
            AnimationCommand::StopTarget(target) => {
                self.effects.remove(&target);
            }
            AnimationCommand::ClearAll => {
                self.effects.clear();
            }
            AnimationCommand::SetFrameRate(fps) => {
                self.frame_rate = fps.max(1);
            }
        }
    }

    fn tick(&mut self, delta_ms: f32) {
        // Tick all effects and remove expired ones
        tracing::debug!("==> controller.tick() starting with {} targets", self.effects.len());
        for (target, effects) in &mut self.effects {
            let before_count = effects.len();
            tracing::debug!("==> Target {:?} has {} effects before tick", target, before_count);
            for effect in effects.iter_mut() {
                tracing::debug!("==> Ticking effect {:?}", effect.id);
                effect.tick(delta_ms);
            }
            tracing::debug!("==> Checking expiration for {} effects", effects.len());
            effects.retain(|e| {
                let expired = e.is_expired();
                if expired {
                    tracing::info!("==> Removing expired effect {:?}", e.id);
                }
                !expired
            });
            let after_count = effects.len();
            tracing::debug!(
                "==> Target {:?} has {} effects after expiration check",
                target,
                after_count
            );
        }
        // Remove empty entries
        self.effects.retain(|_, v| !v.is_empty());
        tracing::debug!("==> controller.tick() finished with {} targets", self.effects.len());
    }

    fn export_state(&self) -> AnimationState {
        AnimationState {
            effects: self.effects.clone(),
        }
    }
}

/// Spawn the animation controller as a background task
///
/// # Arguments
/// * `event_tx` - Channel to send `RenderSignal` events
/// * `frame_rate` - Animation frame rate (fps, default 30)
/// * `state` - Shared state for render access
///
/// # Returns
/// Handle to send commands to the controller
pub fn spawn_animation_controller(
    event_tx: mpsc::Sender<RuntimeEvent>,
    frame_rate: u8,
    state: Arc<RwLock<AnimationState>>,
) -> AnimationHandle {
    let (tx, mut rx) = mpsc::channel::<AnimationCommand>(64);
    let next_id = Arc::new(std::sync::atomic::AtomicU64::new(1));

    tokio::spawn(async move {
        let mut controller = AnimationController::new(frame_rate);
        let mut interval =
            tokio::time::interval(Duration::from_millis(1000 / u64::from(controller.frame_rate)));

        loop {
            tokio::select! {
                // Handle commands (prioritize over ticks)
                Some(cmd) = rx.recv() => {
                    // Check for frame rate change
                    if let AnimationCommand::SetFrameRate(fps) = &cmd {
                        interval = tokio::time::interval(
                            Duration::from_millis(1000 / u64::from(*fps).max(1))
                        );
                    }
                    controller.handle_command(cmd);

                    // Update shared state
                    tracing::debug!("==> Waiting for state write lock (command handler)...");
                    let mut state_guard = state.write().await;
                    tracing::debug!("==> Got state write lock, exporting state");
                    *state_guard = controller.export_state();
                    drop(state_guard);
                    tracing::debug!("==> Releasing state write lock");
                }
                // Tick animations
                _ = interval.tick() => {
                    tracing::debug!("==> Animation interval tick (has_active={}, count={})",
                        controller.has_active_effects(),
                        controller.effects.len());
                    if controller.has_active_effects() {
                        tracing::info!("==> Animation tick: {} active effects", controller.effects.len());
                        let delta_ms = 1000.0 / f32::from(controller.frame_rate);
                        tracing::debug!("==> Calling controller.tick(delta_ms={:.2})", delta_ms);
                        controller.tick(delta_ms);
                        tracing::debug!("==> After tick: {} active effects", controller.effects.len());

                        // Update shared state
                        {
                            tracing::debug!("==> Waiting for state write lock (tick handler)...");
                            let mut state_guard = state.write().await;
                            tracing::debug!("==> Got state write lock, exporting state");
                            *state_guard = controller.export_state();
                            drop(state_guard);
                            tracing::debug!("==> Releasing state write lock");
                        }

                        // Request re-render
                        tracing::debug!("==> Sending RenderSignal");
                        let _ = event_tx.try_send(RuntimeEvent::render_signal());
                        tracing::debug!("==> RenderSignal sent");
                    }
                }
            }
        }
    });

    AnimationHandle { tx, next_id }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::animation::effect::{AnimatedStyle, UiElementId},
    };

    #[test]
    fn test_controller_add_remove() {
        let mut controller = AnimationController::new(30);

        let effect = Effect::new(
            EffectId::new(1),
            EffectTarget::UiElement(UiElementId::Cursor),
            AnimatedStyle::new(),
        );

        controller.handle_command(AnimationCommand::Start(effect));
        assert!(controller.has_active_effects());

        controller.handle_command(AnimationCommand::Stop(EffectId::new(1)));
        assert!(!controller.has_active_effects());
    }

    #[test]
    fn test_controller_clear_all() {
        let mut controller = AnimationController::new(30);

        for i in 0..5 {
            let effect = Effect::new(
                EffectId::new(i),
                EffectTarget::UiElement(UiElementId::Cursor),
                AnimatedStyle::new(),
            );
            controller.handle_command(AnimationCommand::Start(effect));
        }

        assert!(controller.has_active_effects());
        controller.handle_command(AnimationCommand::ClearAll);
        assert!(!controller.has_active_effects());
    }
}
