//! cdylib fixture that intentionally omits the driver vtable symbol.
//!
//! The loader's symbol-lookup path (`lib.get(VTABLE_SYMBOL)`) surfaces
//! `LoadError::LibraryOpen` when the expected static is absent. This
//! fixture is just a linkable cdylib that guarantees the lookup fails.
