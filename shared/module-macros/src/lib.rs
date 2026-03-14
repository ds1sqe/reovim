//! Procedural macros for reovim dynamic module development.
//!
//! This crate provides the `declare_module!` macro for generating FFI-safe
//! entry points required by the kernel's dynamic module loading system.
//!
//! # Linux Kernel Inspiration
//!
//! The generated code follows patterns from Linux kernel modules:
//! - `REOVIM_MODULE_API_VERSION` ≈ `vermagic` string (version check before loading)
//! - `reovim_module_probe()` ≈ `MODULE_INFO` section (metadata without instantiation)
//! - `reovim_module_entry()` ≈ `init_module()` (creates the module instance)
//! - Trampoline functions for FFI-safe lifecycle calls
//!
//! # FFI Safety
//!
//! All generated code uses:
//! - `extern "C"` calling convention
//! - `#[repr(C)]` types
//! - Thin pointers (`*mut c_void`) instead of fat pointers
//! - `catch_unwind` for panic safety
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::api::v1::*;
//! use reovim_module_macros::declare_module;
//!
//! pub struct MyModule {
//!     initialized: bool,
//! }
//!
//! impl Module for MyModule {
//!     fn id(&self) -> ModuleId { ModuleId::new("my-module") }
//!     fn name(&self) -> &'static str { "My Module" }
//!     fn version(&self) -> Version { Version::new(1, 0, 0) }
//!     fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult {
//!         self.initialized = true;
//!         ProbeResult::Success
//!     }
//!     fn exit(&mut self) -> Result<(), ModuleError> { Ok(()) }
//! }
//!
//! impl MyModule {
//!     pub fn new() -> Self { Self { initialized: false } }
//! }
//!
//! declare_module!(MyModule);
//! ```

use {
    proc_macro::TokenStream,
    quote::quote,
    syn::{Ident, parse_macro_input},
};

/// Generates FFI entry points for dynamic module loading.
///
/// # Usage
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
/// use reovim_module_macros::declare_module;
///
/// pub struct MyModule { /* ... */ }
///
/// impl Module for MyModule {
///     fn id(&self) -> ModuleId { ModuleId::new("my-module") }
///     fn name(&self) -> &'static str { "My Module" }
///     fn version(&self) -> Version { Version::new(1, 0, 0) }
///     fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult { ProbeResult::Success }
///     fn exit(&mut self) -> Result<(), ModuleError> { Ok(()) }
/// }
///
/// impl MyModule {
///     pub fn new() -> Self { Self { /* ... */ } }
/// }
///
/// declare_module!(MyModule);
/// ```
///
/// # Generated Symbols
///
/// The macro generates these exported symbols:
///
/// | Symbol | Type | Purpose |
/// |--------|------|---------|
/// | `REOVIM_MODULE_API_VERSION` | `Version` | Pre-load version check |
/// | `reovim_module_probe()` | `fn() -> ModuleProbe` | Metadata query |
/// | `reovim_module_entry()` | `fn() -> *mut c_void` | Instance creation |
/// | `reovim_module_init()` | `fn(*mut c_void, *const c_void) -> i32` | Init trampoline |
/// | `reovim_module_exit()` | `fn(*mut c_void) -> i32` | Exit trampoline |
/// | `reovim_module_destroy()` | `fn(*mut c_void)` | Cleanup trampoline |
///
/// # Loader Protocol
///
/// The module loader should:
/// 1. Load the shared library via `libloading`
/// 2. Read `REOVIM_MODULE_API_VERSION` and check compatibility
/// 3. Call `reovim_module_probe()` to get metadata
/// 4. If compatible, call `reovim_module_entry()` to create instance
/// 5. Call `reovim_module_init()` with instance and context
/// 6. Later, call `reovim_module_exit()` then `reovim_module_destroy()`
///
/// # Requirements
///
/// The module type must:
/// - Implement `Module` trait
/// - Have a `new() -> Self` constructor
/// - Be `Send + Sync + 'static`
///
/// # Return Codes
///
/// Init and exit trampolines return:
/// - `0`: Success
/// - `1`: Defer (init only - try again later)
/// - `-1`: Failed/Error
/// - `-2`: Panic occurred
#[proc_macro]
#[allow(clippy::too_many_lines)] // FFI codegen requires many trampolines
pub fn declare_module(input: TokenStream) -> TokenStream {
    let module_type = parse_macro_input!(input as Ident);

    let expanded = quote! {
        // ====================================================================
        // Static API Version (Linux: vermagic)
        // ====================================================================
        // Loader can read this BEFORE calling any functions to check
        // compatibility. This allows fast rejection of incompatible modules.
        #[unsafe(no_mangle)]
        pub static REOVIM_MODULE_API_VERSION: ::reovim_kernel::api::v1::Version =
            ::reovim_kernel::api::v1::API_VERSION;

        // ====================================================================
        // Module Probe (Linux: MODULE_INFO / .modinfo section)
        // ====================================================================
        // Returns metadata without instantiating the full module.
        // ModuleProbe is #[repr(C)] with fixed-size arrays - FFI-safe.
        #[unsafe(no_mangle)]
        pub extern "C" fn reovim_module_probe() -> ::reovim_kernel::api::v1::ModuleProbe {
            // Create a temporary instance just to query its metadata.
            // This mirrors how Linux MODULE_* macros embed static strings.
            let temp = <#module_type>::new();

            // Build probe from Module trait methods
            let id = {
                use ::reovim_kernel::api::v1::Module;
                temp.id()
            };
            let name = {
                use ::reovim_kernel::api::v1::Module;
                temp.name()
            };
            let version = {
                use ::reovim_kernel::api::v1::Module;
                temp.version()
            };
            let api_version = {
                use ::reovim_kernel::api::v1::Module;
                temp.api_version()
            };
            let required_deps = {
                use ::reovim_kernel::api::v1::Module;
                temp.dependencies()
            };
            let optional_deps = {
                use ::reovim_kernel::api::v1::Module;
                temp.optional_dependencies()
            };

            // Build probe with all metadata
            let mut probe = ::reovim_kernel::api::v1::ModuleProbe::new(
                id.as_str(),
                name,
                version,
                api_version,
            );

            // Add rustc version for ABI compatibility checking
            // RUSTC_VERSION is set by build.rs or defaults to rustc_version crate
            const RUSTC_VERSION: &str = env!("CARGO_PKG_RUST_VERSION");
            probe = probe.with_rustc_version(RUSTC_VERSION);

            // Add required dependencies (up to 8)
            for (i, dep) in required_deps.iter().take(8).enumerate() {
                probe = probe.with_required_dep(i, dep.as_str());
            }

            // Add optional dependencies (up to 8)
            for (i, dep) in optional_deps.iter().take(8).enumerate() {
                probe = probe.with_optional_dep(i, dep.as_str());
            }

            probe
        }

        // ====================================================================
        // Module Entry (Linux: init_module / module_init)
        // ====================================================================
        // Creates module instance and returns thin pointer.
        // Returns *mut c_void (thin pointer), NOT *mut dyn Module (fat pointer).
        //
        // # Safety
        //
        // - Caller must treat returned pointer as opaque
        // - Caller must call reovim_module_init() before using module
        // - Caller must eventually call reovim_module_destroy()
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_module_entry() -> *mut ::std::ffi::c_void {
            let module = ::std::boxed::Box::new(<#module_type>::new());
            ::std::boxed::Box::into_raw(module) as *mut ::std::ffi::c_void
        }

        // ====================================================================
        // Init Trampoline (FFI-safe lifecycle)
        // ====================================================================
        // Calls Module::init() with panic safety.
        //
        // # Returns
        // - 0: Success
        // - 1: Defer (try again later)
        // - -1: Failed
        // - -2: Panic occurred
        //
        // # Safety
        //
        // - `module` must be a pointer from reovim_module_entry()
        // - `ctx` must be a valid ModuleContext pointer
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_module_init(
            module: *mut ::std::ffi::c_void,
            ctx: *const ::std::ffi::c_void,
        ) -> i32 {
            // Catch panics to prevent UB at FFI boundary
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let ctx = &*(ctx as *const ::reovim_kernel::api::v1::ModuleContext);

                use ::reovim_kernel::api::v1::Module;
                module.init(ctx)
            }));

            match result {
                Ok(::reovim_kernel::api::v1::ProbeResult::Success) => 0,
                Ok(::reovim_kernel::api::v1::ProbeResult::Defer(_)) => 1,
                Ok(::reovim_kernel::api::v1::ProbeResult::Failed(_)) => -1,
                Err(_) => -2, // Panic
            }
        }

        // ====================================================================
        // Exit Trampoline (FFI-safe lifecycle)
        // ====================================================================
        // Calls Module::exit() with panic safety.
        //
        // # Returns
        // - 0: Success
        // - -1: Error
        // - -2: Panic occurred
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_module_exit(
            module: *mut ::std::ffi::c_void,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);

                use ::reovim_kernel::api::v1::Module;
                module.exit()
            }));

            match result {
                Ok(Ok(())) => 0,
                Ok(Err(_)) => -1,
                Err(_) => -2, // Panic
            }
        }

        // ====================================================================
        // Destroy Trampoline (FFI-safe cleanup)
        // ====================================================================
        // Drops the module instance and frees memory.
        //
        // # Safety
        //
        // - `module` must be a pointer from reovim_module_entry()
        // - Must not be called twice on the same pointer
        // - Must be called AFTER reovim_module_exit()
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_module_destroy(
            module: *mut ::std::ffi::c_void,
        ) {
            if !module.is_null() {
                // catch_unwind for Drop impl panic safety
                let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                    drop(::std::boxed::Box::from_raw(module as *mut #module_type));
                }));
            }
        }

        // ====================================================================
        // Hot Reload Trampolines (Phase 4.6 addition)
        // ====================================================================

        /// Check if module supports hot reload.
        ///
        /// # Returns
        /// - 1: Supports hot reload
        /// - 0: Does not support hot reload
        /// - -1: Panic occurred
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_module_supports_hot_reload(
            module: *const ::std::ffi::c_void,
        ) -> i32 {
            if module.is_null() {
                return 0;
            }
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_kernel::api::v1::Module;
                module.supports_hot_reload()
            }));

            match result {
                Ok(true) => 1,
                Ok(false) => 0,
                Err(_) => -1, // Panic
            }
        }

        /// Save module state for hot reload.
        ///
        /// # Returns
        /// - 0: Success (state written to out_ptr/out_len)
        /// - 1: No state to save (out_ptr is null, out_len is 0)
        /// - -1: Panic occurred
        ///
        /// # Safety
        /// - Caller must call reovim_module_free_state() on returned pointer
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_module_save_state(
            module: *const ::std::ffi::c_void,
            out_ptr: *mut *mut u8,
            out_len: *mut usize,
        ) -> i32 {
            if module.is_null() || out_ptr.is_null() || out_len.is_null() {
                return -1;
            }

            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_kernel::api::v1::Module;
                module.save_state()
            }));

            match result {
                Ok(Some(data)) => {
                    let len = data.len();
                    let ptr = ::std::boxed::Box::into_raw(data) as *mut u8;
                    *out_ptr = ptr;
                    *out_len = len;
                    0 // Success
                }
                Ok(None) => {
                    *out_ptr = ::std::ptr::null_mut();
                    *out_len = 0;
                    1 // No state
                }
                Err(_) => -1, // Panic
            }
        }

        /// Restore module state after hot reload.
        ///
        /// # Returns
        /// - 0: Success
        /// - -1: Error
        /// - -2: Panic occurred
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_module_restore_state(
            module: *mut ::std::ffi::c_void,
            data: *const u8,
            len: usize,
        ) -> i32 {
            if module.is_null() || (len > 0 && data.is_null()) {
                return -1;
            }

            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let slice = if len > 0 {
                    ::std::slice::from_raw_parts(data, len)
                } else {
                    &[]
                };
                use ::reovim_kernel::api::v1::Module;
                module.restore_state(slice)
            }));

            match result {
                Ok(Ok(())) => 0,
                Ok(Err(_)) => -1,
                Err(_) => -2, // Panic
            }
        }

        /// Free state buffer allocated by save_state.
        ///
        /// # Safety
        /// - `ptr` must be from reovim_module_save_state() or null
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_module_free_state(
            ptr: *mut u8,
            len: usize,
        ) {
            if !ptr.is_null() && len > 0 {
                let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                    drop(::std::boxed::Box::from_raw(
                        ::std::slice::from_raw_parts_mut(ptr, len)
                    ));
                }));
            }
        }
    };

    expanded.into()
}

/// Generates FFI entry points for dynamic **client** module loading.
///
/// This is the client-side counterpart to [`declare_module!`]. It generates
/// symbols prefixed with `reovim_client_module_*` to avoid collision with
/// server-side symbols when both module types exist in the same `.so`.
///
/// # Usage
///
/// ```ignore
/// use reovim_client_driver::*;
/// use reovim_module_macros::declare_client_module;
///
/// pub struct MyClientModule { /* ... */ }
///
/// impl ClientModule for MyClientModule {
///     fn id(&self) -> &'static str { "my-module" }
///     fn kind(&self) -> &'static str { "my-module" }
///     fn name(&self) -> &'static str { "My Module" }
///     fn version(&self) -> Version { Version::new(1, 0, 0) }
///     fn init(&mut self, _ctx: &ModuleContext) -> ProbeResult { ProbeResult::Success }
///     fn exit(&mut self) -> Result<(), ClientModuleError> { Ok(()) }
/// }
///
/// impl MyClientModule {
///     pub fn new() -> Self { Self { /* ... */ } }
/// }
///
/// declare_client_module!(MyClientModule);
/// ```
///
/// # Generated Symbols
///
/// | Symbol | Type | Purpose |
/// |--------|------|---------|
/// | `REOVIM_CLIENT_MODULE_API_VERSION` | `Version` | Pre-load version check |
/// | `reovim_client_module_probe()` | `fn() -> ClientModuleProbe` | Metadata query |
/// | `reovim_client_module_entry()` | `fn() -> *mut c_void` | Instance creation |
/// | `reovim_client_module_init()` | `fn(*mut c_void, *const c_void) -> i32` | Init trampoline |
/// | `reovim_client_module_exit()` | `fn(*mut c_void) -> i32` | Exit trampoline |
/// | `reovim_client_module_destroy()` | `fn(*mut c_void)` | Cleanup |
/// | `reovim_client_module_on_all_loaded()` | `fn(*mut c_void, *const c_void)` | Lifecycle hook |
///
/// # Requirements
///
/// The module type must:
/// - Implement `ClientModule` trait
/// - Have a `new() -> Self` constructor
/// - Be `Send + Sync + 'static`
///
/// # Return Codes
///
/// Init and exit trampolines return:
/// - `0`: Success
/// - `1`: Defer (init only - try again later)
/// - `-1`: Failed/Error
/// - `-2`: Panic occurred
#[proc_macro]
#[allow(clippy::too_many_lines)]
pub fn declare_client_module(input: TokenStream) -> TokenStream {
    let module_type = parse_macro_input!(input as Ident);

    let expanded = quote! {
        // ====================================================================
        // Static API Version
        // ====================================================================
        #[unsafe(no_mangle)]
        pub static REOVIM_CLIENT_MODULE_API_VERSION: ::reovim_client_driver::Version =
            ::reovim_client_driver::CLIENT_MODULE_API_VERSION;

        // ====================================================================
        // Module Probe (metadata without instantiation)
        // ====================================================================
        #[unsafe(no_mangle)]
        pub extern "C" fn reovim_client_module_probe() -> ::reovim_client_driver::ClientModuleProbe {
            let temp = <#module_type>::new();

            let id = {
                use ::reovim_client_driver::ClientModule;
                temp.id()
            };
            let name = {
                use ::reovim_client_driver::ClientModule;
                temp.name()
            };
            let version = {
                use ::reovim_client_driver::ClientModule;
                temp.version()
            };

            let required_deps = {
                use ::reovim_client_driver::ClientModule;
                temp.dependencies()
            };
            let optional_deps = {
                use ::reovim_client_driver::ClientModule;
                temp.optional_dependencies()
            };

            let mut probe = ::reovim_client_driver::ClientModuleProbe::new(
                id,
                name,
                version,
                ::reovim_client_driver::CLIENT_MODULE_API_VERSION,
            );

            for (i, dep) in required_deps.iter().take(8).enumerate() {
                probe = probe.with_required_dep(i, dep);
            }

            for (i, dep) in optional_deps.iter().take(8).enumerate() {
                probe = probe.with_optional_dep(i, dep);
            }

            probe
        }

        // ====================================================================
        // Module Entry (instance creation)
        // ====================================================================
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_entry() -> *mut ::std::ffi::c_void {
            let module = ::std::boxed::Box::new(<#module_type>::new());
            ::std::boxed::Box::into_raw(module) as *mut ::std::ffi::c_void
        }

        // ====================================================================
        // Init Trampoline
        // ====================================================================
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_init(
            module: *mut ::std::ffi::c_void,
            ctx: *const ::std::ffi::c_void,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let ctx = &*(ctx as *const ::reovim_client_driver::ModuleContext);

                use ::reovim_client_driver::ClientModule;
                module.init(ctx)
            }));

            match result {
                Ok(::reovim_client_driver::ProbeResult::Success) => 0,
                Ok(::reovim_client_driver::ProbeResult::Defer(_)) => 1,
                Ok(::reovim_client_driver::ProbeResult::Failed(_)) => -1,
                Err(_) => -2,
            }
        }

        // ====================================================================
        // Exit Trampoline
        // ====================================================================
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_exit(
            module: *mut ::std::ffi::c_void,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);

                use ::reovim_client_driver::ClientModule;
                module.exit()
            }));

            match result {
                Ok(Ok(())) => 0,
                Ok(Err(_)) => -1,
                Err(_) => -2,
            }
        }

        // ====================================================================
        // Destroy Trampoline
        // ====================================================================
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_destroy(
            module: *mut ::std::ffi::c_void,
        ) {
            if !module.is_null() {
                let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                    drop(::std::boxed::Box::from_raw(module as *mut #module_type));
                }));
            }
        }

        // ====================================================================
        // On All Loaded Trampoline
        // ====================================================================
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_on_all_loaded(
            module: *mut ::std::ffi::c_void,
            ctx: *const ::std::ffi::c_void,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let ctx = &*(ctx as *const ::reovim_client_driver::ModuleContext);

                use ::reovim_client_driver::ClientModule;
                module.on_all_loaded(ctx);
            }));
        }
    };

    expanded.into()
}
