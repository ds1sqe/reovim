use super::*;

use std::{collections::HashMap, path::PathBuf};

use crate::{
    api::module::ModuleId,
    mm::{BufferId, Cursor, Position, WindowId},
};

// Text types extracted to reovim-types-text (#740)
use reovim_types_text::{
    Direction, HistoryRing, LinePosition, Motion, MotionEngine, Register, RegisterBank,
    RegisterContent, SimpleText, TextObject, TextObjectEngine, WordBoundary, YankType,
};

mod config;
mod direction;
mod history;
mod jumplist;
mod mark;
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

#[path = "../text_geometry_tests.rs"]
mod text_geometry;

mod b9_repro;
