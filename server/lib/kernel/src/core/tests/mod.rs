use super::*;

use std::{collections::HashMap, path::PathBuf};

use crate::{
    api::module::ModuleId,
    mm::{Buffer, BufferId, Cursor, Position, WindowId},
};

mod config;
mod direction;
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
