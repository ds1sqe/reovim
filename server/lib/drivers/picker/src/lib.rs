//! Picker driver for reovim.
//!
//! This driver defines the interface for pluggable fuzzy finder data sources.
//! Following the mechanism/policy separation:
//!
//! - **Mechanism** (this driver): `Picker` trait, types, `PickerRegistry`, `PickerEngine`
//! - **Policy** (modules): Implementations like `FilesPicker`, `BuffersPicker`, `GrepPicker`
//!
//! # Architecture
//!
//! ```text
//! server/lib/drivers/picker/      -> Trait + Types + Registry + Engine (MECHANISM)
//! server/modules/microscope/      -> Picker implementations + orchestration (POLICY)
//! ```

mod action;
mod context;
mod engine;
mod item;
mod picker;
mod preview;
mod registry;

pub use {
    action::PickerAction,
    context::{BufferInfo, CommandInfo, OptionInfo, PickerContext},
    engine::{EngineItem, PickerEngine, TickStatus, push_item, push_items},
    item::{PickerData, PickerItem, file_type_icon, icon_for_path},
    picker::Picker,
    preview::{PreviewContent, PreviewHighlight},
    registry::PickerRegistry,
    reovim_driver_session::SessionRuntime,
};
