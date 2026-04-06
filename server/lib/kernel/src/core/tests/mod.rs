use super::*;

use std::{collections::HashMap, path::PathBuf};

use crate::{
    api::module::ModuleId,
    mm::{BufferId, Cursor, WindowId},
};

use reovim_types_text::Position;

// Text types extracted to reovim-types-text (#740)
use reovim_types_text::{
    Direction, HistoryRing, LinePosition, Motion, MotionEngine, Register, RegisterBank,
    RegisterContent, SimpleText, TextObject, TextObjectEngine, WordBoundary, YankType,
};

mod config;
mod direction;
mod history;
// Mark tests moved to reovim-driver-session (#740)
mod mode;
mod motion_engine;
mod motion_types;
mod option_constraint;
mod option_error;
mod option_mod;
mod option_scope;
mod option_spec;
mod option_value;
mod register;
mod textobj;

mod b9_repro;
