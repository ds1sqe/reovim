//! Toast notification and progress bar plugin for reovim
//!
//! This plugin provides non-blocking notifications and progress indicators.
//!
//! # Usage
//!
//! Other plugins can emit notification events:
//!
//! ```ignore
//! use reovim_plugin_notification::{NotificationShow, ProgressUpdate};
//!
//! // Show a notification
//! bus.emit(NotificationShow::success("File saved"));
//!
//! // Show progress
//! bus.emit(ProgressUpdate::new("build", "Building", "cargo").with_progress(45));
//! ```

pub mod command;
pub mod notification;
pub mod progress;
pub mod state;
pub mod style;
pub mod window;

use std::{any::TypeId, sync::Arc};

use reovim_core::{
    event_bus::{EventBus, EventResult},
    plugin::{Plugin, PluginContext, PluginId, PluginStateRegistry},
    rpc::{RpcHandler, RpcHandlerContext, RpcResult},
};

use {
    command::{NotificationDismiss, NotificationDismissAll, NotificationShow},
    state::SharedNotificationManager,
    window::NotificationPluginWindow,
};

// Re-export for external use
pub use {
    command::{
        NotificationDismissAll as DismissAll, NotificationShow as Show, ProgressComplete,
        ProgressUpdate,
    },
    notification::{Notification, NotificationLevel},
    progress::{ProgressBarConfig, ProgressNotification},
    state::{NotificationConfig, NotificationManagerHandle, NotificationPosition},
    style::NotificationStyles,
};

/// RPC handler for state/notification queries
struct NotificationStateHandler;

impl RpcHandler for NotificationStateHandler {
    fn method(&self) -> &'static str {
        "state/notification"
    }

    fn handle(&self, _params: &serde_json::Value, ctx: &RpcHandlerContext) -> RpcResult {
        let state = ctx
            .with_state::<Arc<SharedNotificationManager>, _, _>(|manager| {
                let notifications = manager.notifications();
                let progress_items = manager.progress_items();

                serde_json::json!({
                    "notification_count": notifications.len(),
                    "progress_count": progress_items.len(),
                    "has_visible": manager.has_visible(),
                    "notifications": notifications.iter().map(|n| {
                        serde_json::json!({
                            "id": n.id,
                            "message": n.message,
                            "level": format!("{:?}", n.level),
                            "source": n.source,
                        })
                    }).collect::<Vec<_>>(),
                    "progress": progress_items.iter().map(|p| {
                        serde_json::json!({
                            "id": p.id,
                            "title": p.title,
                            "source": p.source,
                            "progress": p.progress,
                            "detail": p.detail,
                        })
                    }).collect::<Vec<_>>(),
                })
            })
            .unwrap_or_else(|| {
                serde_json::json!({
                    "notification_count": 0,
                    "progress_count": 0,
                    "has_visible": false,
                    "notifications": [],
                    "progress": [],
                })
            });
        RpcResult::Success(state)
    }

    fn description(&self) -> &'static str {
        "Get notification plugin state"
    }
}

/// RPC handler for triggering test notifications
struct NotificationShowHandler;

impl RpcHandler for NotificationShowHandler {
    fn method(&self) -> &'static str {
        "notification/show"
    }

    fn handle(&self, params: &serde_json::Value, ctx: &RpcHandlerContext) -> RpcResult {
        let message = params
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Test notification");

        let level = match params.get("level").and_then(serde_json::Value::as_str) {
            Some("success") => notification::NotificationLevel::Success,
            Some("warning") => notification::NotificationLevel::Warning,
            Some("error") => notification::NotificationLevel::Error,
            _ => notification::NotificationLevel::Info,
        };

        let duration_ms = params
            .get("duration_ms")
            .and_then(serde_json::Value::as_u64);

        let source = params
            .get("source")
            .and_then(serde_json::Value::as_str)
            .map(String::from);

        let result = ctx.with_state::<Arc<SharedNotificationManager>, _, _>(|manager| {
            manager.show(level, message.to_string(), duration_ms, source);
            true
        });

        if result.unwrap_or(false) {
            RpcResult::ok()
        } else {
            RpcResult::internal_error("Notification manager not available")
        }
    }

    fn description(&self) -> &'static str {
        "Show a test notification"
    }
}

/// RPC handler for triggering test progress updates
struct ProgressUpdateHandler;

impl RpcHandler for ProgressUpdateHandler {
    fn method(&self) -> &'static str {
        "notification/progress"
    }

    fn handle(&self, params: &serde_json::Value, ctx: &RpcHandlerContext) -> RpcResult {
        let id = params
            .get("id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("test")
            .to_string();

        let title = params
            .get("title")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("Test Progress")
            .to_string();

        let source = params
            .get("source")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("test")
            .to_string();

        #[allow(clippy::cast_possible_truncation)]
        let progress = params
            .get("progress")
            .and_then(serde_json::Value::as_u64)
            .map(|p| p as u8);

        let detail = params
            .get("detail")
            .and_then(serde_json::Value::as_str)
            .map(String::from);

        let result = ctx.with_state::<Arc<SharedNotificationManager>, _, _>(|manager| {
            manager.update_progress(id, title, source, progress, detail);
            true
        });

        if result.unwrap_or(false) {
            RpcResult::ok()
        } else {
            RpcResult::internal_error("Notification manager not available")
        }
    }

    fn description(&self) -> &'static str {
        "Update a progress notification"
    }
}

/// Notification plugin
///
/// Provides toast notifications and progress bars.
pub struct NotificationPlugin {
    manager: Arc<SharedNotificationManager>,
}

impl Default for NotificationPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationPlugin {
    /// Create a new notification plugin
    #[must_use]
    pub fn new() -> Self {
        Self {
            manager: Arc::new(SharedNotificationManager::new()),
        }
    }
}

impl Plugin for NotificationPlugin {
    fn id(&self) -> PluginId {
        PluginId::new("reovim:notification")
    }

    fn name(&self) -> &'static str {
        "Notification"
    }

    fn description(&self) -> &'static str {
        "Toast notifications and progress bars"
    }

    fn dependencies(&self) -> Vec<TypeId> {
        vec![]
    }

    fn build(&self, ctx: &mut PluginContext) {
        // Register dismiss all command
        let _ = ctx.register_command(NotificationDismissAll);

        // Register RPC handlers for testing
        ctx.register_rpc_handler(Arc::new(NotificationStateHandler));
        ctx.register_rpc_handler(Arc::new(NotificationShowHandler));
        ctx.register_rpc_handler(Arc::new(ProgressUpdateHandler));

        tracing::debug!("NotificationPlugin: registered commands and RPC handlers");
    }

    fn init_state(&self, registry: &PluginStateRegistry) {
        // Register the manager in plugin state
        registry.register(Arc::clone(&self.manager));

        // Register the plugin window (it accesses manager from the registry)
        registry.register_plugin_window(Arc::new(NotificationPluginWindow::new()));

        tracing::debug!("NotificationPlugin: registered window and state");
    }

    fn subscribe(&self, bus: &EventBus, _state: Arc<PluginStateRegistry>) {
        // Subscribe to show notification events
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<NotificationShow, _>(100, move |event, ctx| {
            manager.show(
                event.level,
                event.message.clone(),
                event.duration_ms,
                event.source.clone(),
            );
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to dismiss events
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<NotificationDismiss, _>(100, move |event, ctx| {
            manager.dismiss(&event.id);
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to dismiss all command
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<NotificationDismissAll, _>(100, move |_event, ctx| {
            manager.dismiss_all();
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to progress update events
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<ProgressUpdate, _>(100, move |event, ctx| {
            manager.update_progress(
                event.id.clone(),
                event.title.clone(),
                event.source.clone(),
                event.progress,
                event.detail.clone(),
            );
            ctx.request_render();
            EventResult::Handled
        });

        // Subscribe to progress complete events
        let manager = Arc::clone(&self.manager);
        bus.subscribe::<ProgressComplete, _>(100, move |event, ctx| {
            manager.complete_progress(&event.id, event.message.clone());
            ctx.request_render();
            EventResult::Handled
        });

        tracing::debug!("NotificationPlugin: subscribed to events");
    }
}
