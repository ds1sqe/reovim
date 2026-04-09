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
/// | `reovim_module_on_all_loaded()` | `fn(*mut c_void, *const c_void)` | Lifecycle hook |
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

        // ====================================================================
        // on_all_loaded Trampoline (#725 — lifecycle hook)
        // ====================================================================
        //
        // Mirrors the client-side `reovim_client_module_on_all_loaded` pattern.
        // Called by the host after all modules in the same pass have been
        // initialized, so modules can look up services contributed by their
        // dependencies.
        //
        // # Safety
        //
        // - `module` must be a pointer returned by `reovim_module_entry()`
        // - `ctx` must be a valid `*const ModuleContext` for the duration of
        //   the call
        //
        // Panics inside the module are caught at the FFI boundary and swallowed
        // (the host logs via the dynamic dispatch site). This mirrors the
        // `on_all_loaded` error-handling contract used by the trait default.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_module_on_all_loaded(
            module: *mut ::std::ffi::c_void,
            ctx: *const ::std::ffi::c_void,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let ctx = &*(ctx as *const ::reovim_kernel::api::v1::ModuleContext);

                use ::reovim_kernel::api::v1::Module;
                module.on_all_loaded(ctx);
            }));
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

            // Populate capability flags from trait methods.
            let has_chrome = {
                use ::reovim_client_driver::ClientModule;
                temp.has_chrome()
            };
            let has_buffer_contrib = {
                use ::reovim_client_driver::ClientModule;
                temp.has_buffer_contrib()
            };
            let has_annotations = {
                use ::reovim_client_driver::ClientModule;
                temp.has_annotations()
            };
            probe = probe.with_capabilities(has_chrome, has_buffer_contrib, has_annotations);

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

        // ====================================================================
        // Event Trampolines (0.3.0)
        // ====================================================================

        /// On-notification trampoline. Converts ptr+len to `&str` and calls
        /// `ClientModule::on_notification`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_on_notification(
            module: *mut ::std::ffi::c_void,
            data_ptr: *const u8,
            data_len: usize,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let data = ::std::str::from_utf8(
                    ::std::slice::from_raw_parts(data_ptr, data_len),
                )
                .unwrap_or("");
                use ::reovim_client_driver::ClientModule;
                module.on_notification(data);
            }));
        }

        /// On-mode-change trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_on_mode_change(
            module: *mut ::std::ffi::c_void,
            mode_ptr: *const u8,
            mode_len: usize,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let mode = ::std::str::from_utf8(
                    ::std::slice::from_raw_parts(mode_ptr, mode_len),
                )
                .unwrap_or("");
                use ::reovim_client_driver::ClientModule;
                module.on_mode_change(mode);
            }));
        }

        /// On-cursor-update trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_on_cursor_update(
            module: *mut ::std::ffi::c_void,
            buffer_id: usize,
            line: usize,
            col: usize,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                use ::reovim_client_driver::ClientModule;
                module.on_cursor_update(
                    ::reovim_client_driver::BufferId(buffer_id),
                    line,
                    col,
                );
            }));
        }

        /// On-buffer-focus trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_on_buffer_focus(
            module: *mut ::std::ffi::c_void,
            buffer_id: usize,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                use ::reovim_client_driver::ClientModule;
                module.on_buffer_focus(::reovim_client_driver::BufferId(buffer_id));
            }));
        }

        /// On-buffer-update trampoline. Passes scalar metadata only; `new_lines`
        /// is empty (dynamic modules re-fetch content via `ServerHandle`).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_on_buffer_update(
            module: *mut ::std::ffi::c_void,
            buffer_id: usize,
            revision: u64,
            changed_start: usize,
            changed_end: usize,
            total_lines: usize,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let event = ::reovim_client_driver::BufferUpdateEvent {
                    buffer_id: ::reovim_client_driver::BufferId(buffer_id),
                    revision,
                    changed_range: changed_start..changed_end,
                    new_lines: ::std::vec::Vec::new(),
                    total_lines,
                };
                use ::reovim_client_driver::ClientModule;
                module.on_buffer_update(&event);
            }));
        }

        /// On-option-changed trampoline. Decodes tag+value into `OptionValue`.
        ///
        /// Tag encoding: 0 = Bool (i64 != 0), 1 = Integer (i64), 2 = String.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_on_option_changed(
            module: *mut ::std::ffi::c_void,
            name_ptr: *const u8,
            name_len: usize,
            tag: i32,
            i64_val: i64,
            str_ptr: *const u8,
            str_len: usize,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let name = ::std::str::from_utf8(
                    ::std::slice::from_raw_parts(name_ptr, name_len),
                )
                .unwrap_or("");
                let value = match tag {
                    0 => ::reovim_client_driver::OptionValue::Bool(i64_val != 0),
                    1 => ::reovim_client_driver::OptionValue::Integer(i64_val),
                    2 => {
                        let s = ::std::str::from_utf8(
                            ::std::slice::from_raw_parts(str_ptr, str_len),
                        )
                        .unwrap_or("")
                        .to_string();
                        ::reovim_client_driver::OptionValue::String(s)
                    }
                    _ => return, // unknown tag — ignore
                };
                use ::reovim_client_driver::ClientModule;
                module.on_option_changed(name, &value);
            }));
        }

        /// Tick trampoline. Returns 1 if redraw needed, 0 otherwise, -2 on panic.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_tick(
            module: *mut ::std::ffi::c_void,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                use ::reovim_client_driver::ClientModule;
                module.tick()
            }));
            match result {
                Ok(true) => 1,
                Ok(false) => 0,
                Err(_) => -2,
            }
        }

        // ====================================================================
        // Role Declaration Trampolines (0.3.0)
        // ====================================================================

        /// Has-chrome trampoline. Returns 1 if true, 0 if false, -2 on panic.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_has_chrome(
            module: *mut ::std::ffi::c_void,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                module.has_chrome()
            }));
            match result {
                Ok(true) => 1,
                Ok(false) => 0,
                Err(_) => -2,
            }
        }

        /// Has-buffer-contrib trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_has_buffer_contrib(
            module: *mut ::std::ffi::c_void,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                module.has_buffer_contrib()
            }));
            match result {
                Ok(true) => 1,
                Ok(false) => 0,
                Err(_) => -2,
            }
        }

        /// Has-annotations trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_has_annotations(
            module: *mut ::std::ffi::c_void,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                module.has_annotations()
            }));
            match result {
                Ok(true) => 1,
                Ok(false) => 0,
                Err(_) => -2,
            }
        }

        /// Chrome-position trampoline. Returns encoded integer:
        /// 0=Top, 1=Bottom, 2=Left, 3=Right, 4=Overlay.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_chrome_position(
            module: *mut ::std::ffi::c_void,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                module.chrome_position()
            }));
            match result {
                Ok(::reovim_client_driver::ChromePosition::Top) => 0,
                Ok(::reovim_client_driver::ChromePosition::Bottom) => 1,
                Ok(::reovim_client_driver::ChromePosition::Left) => 2,
                Ok(::reovim_client_driver::ChromePosition::Right) => 3,
                Ok(::reovim_client_driver::ChromePosition::Overlay) => 4,
                Err(_) => 1, // default: Bottom on panic
            }
        }

        /// Chrome-requested-size trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_chrome_requested_size(
            module: *mut ::std::ffi::c_void,
        ) -> u16 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                // Use a minimal NullCaps for the caps parameter. Dynamic modules
                // should use capabilities cached from init().
                struct NullCaps;
                impl ::reovim_client_driver::PlatformCapabilities for NullCaps {
                    fn rendering_model(&self) -> ::reovim_client_driver::RenderingModel {
                        ::reovim_client_driver::RenderingModel::CellGrid
                    }
                    fn grid_size(&self) -> Option<(u16, u16)> { None }
                    fn color_depth(&self) -> ::reovim_client_driver::ColorDepth {
                        ::reovim_client_driver::ColorDepth::TrueColor
                    }
                    fn pixel_size(&self) -> Option<(u32, u32)> { None }
                    fn reliable_unicode_width(&self) -> bool { true }
                    fn dark_mode(&self) -> bool { false }
                    fn smooth_scroll(&self) -> bool { false }
                    fn pointer_events(&self) -> bool { false }
                    fn touch_input(&self) -> bool { false }
                    fn haptic(&self) -> bool { false }
                    fn safe_area(&self) -> ::reovim_client_driver::Insets {
                        ::reovim_client_driver::Insets::ZERO
                    }
                    fn has_focus(&self) -> bool { true }
                    fn clipboard_available(&self) -> bool { false }
                    fn screen_reader_active(&self) -> bool { false }
                }
                module.chrome_requested_size(&NullCaps)
            }));
            result.unwrap_or(1)
        }

        /// Chrome-priority trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_chrome_priority(
            module: *mut ::std::ffi::c_void,
        ) -> u16 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                module.chrome_priority()
            }));
            result.unwrap_or(0)
        }

        /// Chrome-z-order trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_chrome_z_order(
            module: *mut ::std::ffi::c_void,
        ) -> u16 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                module.chrome_z_order()
            }));
            result.unwrap_or(0)
        }

        /// Buffer-contrib-priority trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_buffer_contrib_priority(
            module: *mut ::std::ffi::c_void,
        ) -> u16 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                module.buffer_contrib_priority()
            }));
            result.unwrap_or(0)
        }

        /// Annotation-priority trampoline.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_annotation_priority(
            module: *mut ::std::ffi::c_void,
        ) -> u16 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                module.annotation_priority()
            }));
            result.unwrap_or(0)
        }

        // ====================================================================
        // Render Trampolines (#723 — API 0.4.0)
        // ====================================================================
        //
        // All render trampolines follow the same pattern:
        //   1. catch_unwind(AssertUnwindSafe(...)) around the entire body
        //   2. Cast module pointer to concrete type
        //   3. Convert FFI args -> Rust args (vtable refs, context decodes)
        //   4. Call the trait method
        //   5. Marshal return -> FFI representation (sentinel for None,
        //      out-pointer + thread-local for slices, heap for transformed
        //      lines)
        //
        // # Backward compatibility
        //
        // Every trampoline is individually resolved with `.ok()` on the host
        // side — old `.so` files compiled against 0.3.0 simply lack these
        // symbols and the handle's dispatch methods fall through to trait
        // defaults. Host code observes the same behavior either way.

        /// Chrome render trampoline (#723).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_chrome_render(
            module: *mut ::std::ffi::c_void,
            surface: *mut ::reovim_client_driver::ffi::FfiRenderSurface,
            bounds: ::reovim_client_driver::Rect,
            caps: *const ::reovim_client_driver::ffi::FfiPlatformCaps,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                let mut surface_ref =
                    ::reovim_client_driver::ffi::FfiRenderSurfaceRef::new(&mut *surface);
                let caps_snap = &*caps;
                let caps_impl =
                    ::reovim_client_driver::ffi::FfiCapsImpl::new(caps_snap);
                use ::reovim_client_driver::ClientModule;
                module.chrome_render(&mut surface_ref, bounds, &caps_impl);
            }));
        }

        /// Annotate trampoline (#723). Returns an FFI gutter cell; the
        /// sentinel `FfiGutterCell::NONE` (text_len == 0) represents `None`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_annotate(
            module: *mut ::std::ffi::c_void,
            line: usize,
            ctx: *const ::reovim_client_driver::ffi::FfiAnnotationContext,
        ) -> ::reovim_client_driver::ffi::FfiGutterCell {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                let ctx_owned = (*ctx).into_ctx();
                use ::reovim_client_driver::ClientModule;
                module
                    .annotate(line, &ctx_owned)
                    .as_ref()
                    .map_or(
                        ::reovim_client_driver::ffi::FfiGutterCell::NONE,
                        ::reovim_client_driver::ffi::FfiGutterCell::from_cell,
                    )
            }));
            result.unwrap_or(::reovim_client_driver::ffi::FfiGutterCell::NONE)
        }

        /// Annotation column width trampoline (#723).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_annotation_column_width(
            module: *mut ::std::ffi::c_void,
            ctx: *const ::reovim_client_driver::ffi::FfiAnnotationContext,
            caps: *const ::reovim_client_driver::ffi::FfiPlatformCaps,
        ) -> ::reovim_client_driver::ffi::FfiColumnWidth {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                let ctx_owned = (*ctx).into_ctx();
                let caps_snap = &*caps;
                let caps_impl =
                    ::reovim_client_driver::ffi::FfiCapsImpl::new(caps_snap);
                use ::reovim_client_driver::ClientModule;
                let w = module.annotation_column_width(&ctx_owned, &caps_impl);
                ::reovim_client_driver::ffi::FfiColumnWidth::from_width(w)
            }));
            result.unwrap_or(::reovim_client_driver::ffi::FfiColumnWidth::ZERO)
        }

        /// Transform line trampoline (#723).
        ///
        /// Returns a pointer to a heap-allocated `FfiTransformedLine`
        /// owned by the module's `.so`. The host must call
        /// `reovim_client_module_free_transformed_line` on the pointer
        /// after reading the fields.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_transform_line(
            module: *mut ::std::ffi::c_void,
            buf: usize,
            line: usize,
            text_ptr: *const u8,
            text_len: usize,
        ) -> *mut ::reovim_client_driver::ffi::FfiTransformedLine {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                // SAFETY: text_ptr/text_len come from a Rust &str on the
                // host side (UTF-8 by type system).
                let text = ::std::str::from_utf8_unchecked(
                    ::std::slice::from_raw_parts(text_ptr, text_len),
                );
                use ::reovim_client_driver::ClientModule;
                #[allow(clippy::option_if_let_else)]
                match module.transform_line(
                    ::reovim_client_driver::BufferId(buf),
                    line,
                    text,
                ) {
                    Some(tl) => {
                        #[allow(deprecated)]
                        ::reovim_client_driver::ffi::FfiTransformedLine::from_rust(tl)
                    }
                    None => ::std::ptr::null_mut(),
                }
            }));
            result.unwrap_or(::std::ptr::null_mut())
        }

        /// Free a transformed line allocated by this module (#723).
        ///
        /// **Must only be called with pointers returned by this module's
        /// own `reovim_client_module_transform_line` trampoline.** The
        /// free runs in module context to match the allocator that
        /// created the heap blocks.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_free_transformed_line(
            ptr: *mut ::reovim_client_driver::ffi::FfiTransformedLine,
        ) {
            if !ptr.is_null() {
                let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                    #[allow(deprecated)]
                    ::reovim_client_driver::ffi::FfiTransformedLine::free(ptr);
                }));
            }
        }

        /// Map cursor column trampoline (#723).
        ///
        /// Returns -1 for `None`, otherwise the u16 value as i32.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_map_cursor_column(
            module: *mut ::std::ffi::c_void,
            buf: usize,
            line: usize,
            col: usize,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                module
                    .map_cursor_column(::reovim_client_driver::BufferId(buf), line, col)
                    .map_or(-1_i32, i32::from)
            }));
            result.unwrap_or(-1)
        }

        /// Fold ranges trampoline (#723).
        ///
        /// Writes `out_ptr`/`out_len` from a thread-local `Vec<FfiFoldRange>`
        /// cache. The output is valid until the next call to this function
        /// on the same thread (single-threaded render path invariant).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_fold_ranges(
            module: *mut ::std::ffi::c_void,
            out_ptr: *mut *const ::reovim_client_driver::ffi::FfiFoldRange,
            out_len: *mut usize,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                let ranges = module.fold_ranges();

                // SAFETY on the RefCell below:
                //   1. Thread-local storage — never crosses threads.
                //   2. `catch_unwind` wraps this block exactly once per FFI
                //      invocation; there is no retry on the same thread.
                //   3. Drop-safe: if the trait method panics mid-iteration,
                //      the `RefMut` guard releases during unwinding before
                //      `catch_unwind` catches.
                ::std::thread_local! {
                    static CACHE: ::std::cell::RefCell<
                        ::std::vec::Vec<::reovim_client_driver::ffi::FfiFoldRange>
                    > = const { ::std::cell::RefCell::new(::std::vec::Vec::new()) };
                }
                CACHE.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    cache.clear();
                    cache.extend(ranges.iter().map(|(start_line, line_count)| {
                        ::reovim_client_driver::ffi::FfiFoldRange {
                            start_line: *start_line,
                            line_count: *line_count,
                        }
                    }));
                    *out_ptr = cache.as_ptr();
                    *out_len = cache.len();
                });
            }));
        }

        /// Virtual lines trampoline (#723).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_virtual_lines(
            module: *mut ::std::ffi::c_void,
            out_ptr: *mut *const ::reovim_client_driver::ffi::FfiVirtualLine,
            out_len: *mut usize,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                let lines = module.virtual_lines();

                // SAFETY: see fold_ranges RefCell rationale above.
                ::std::thread_local! {
                    static CACHE: ::std::cell::RefCell<
                        ::std::vec::Vec<::reovim_client_driver::ffi::FfiVirtualLine>
                    > = const { ::std::cell::RefCell::new(::std::vec::Vec::new()) };
                }
                CACHE.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    cache.clear();
                    cache.extend(
                        lines
                            .iter()
                            .map(::reovim_client_driver::ffi::FfiVirtualLine::from_rust),
                    );
                    *out_ptr = cache.as_ptr();
                    *out_len = cache.len();
                });
            }));
        }

        /// Inline decorations trampoline (#723).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_inline_decorations(
            module: *mut ::std::ffi::c_void,
            line: usize,
            out_ptr: *mut *const ::reovim_client_driver::ffi::FfiInlineDecoration,
            out_len: *mut usize,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                let decos = module.inline_decorations(line);

                // SAFETY: see fold_ranges RefCell rationale above.
                ::std::thread_local! {
                    static CACHE: ::std::cell::RefCell<
                        ::std::vec::Vec<::reovim_client_driver::ffi::FfiInlineDecoration>
                    > = const { ::std::cell::RefCell::new(::std::vec::Vec::new()) };
                }
                CACHE.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    cache.clear();
                    cache.extend(
                        decos
                            .iter()
                            .map(::reovim_client_driver::ffi::FfiInlineDecoration::from_deco),
                    );
                    *out_ptr = cache.as_ptr();
                    *out_len = cache.len();
                });
            }));
        }

        /// Cursor position trampoline (#723).
        ///
        /// Writes into `out_col`/`out_row` and returns 1 if a cursor
        /// position was set, 0 otherwise.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_cursor_position(
            module: *mut ::std::ffi::c_void,
            w: u16,
            h: u16,
            out_col: *mut u16,
            out_row: *mut u16,
        ) -> i32 {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                use ::reovim_client_driver::ClientModule;
                if let Some((c, r)) = module.cursor_position(w, h) {
                    *out_col = c;
                    *out_row = r;
                    1_i32
                } else {
                    0_i32
                }
            }));
            result.unwrap_or(0)
        }

        /// Classify token trampoline (#723).
        ///
        /// Returns `FfiRenderBehavior::NONE` (tag 255) for `None`.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_classify_token(
            module: *mut ::std::ffi::c_void,
            cat_ptr: *const u8,
            cat_len: usize,
        ) -> ::reovim_client_driver::ffi::FfiRenderBehavior {
            let result = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &*(module as *const #module_type);
                // SAFETY: cat_ptr/cat_len come from a Rust &str on the host.
                let category = ::std::str::from_utf8_unchecked(
                    ::std::slice::from_raw_parts(cat_ptr, cat_len),
                );
                use ::reovim_client_driver::ClientModule;
                ::reovim_client_driver::ffi::FfiRenderBehavior::from_option(
                    module.classify_token(category),
                )
            }));
            result.unwrap_or(::reovim_client_driver::ffi::FfiRenderBehavior::NONE)
        }

        /// On capabilities changed trampoline (#723).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_on_capabilities_changed(
            module: *mut ::std::ffi::c_void,
            caps: *const ::reovim_client_driver::ffi::FfiPlatformCaps,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let caps_snap = &*caps;
                let caps_impl =
                    ::reovim_client_driver::ffi::FfiCapsImpl::new(caps_snap);
                use ::reovim_client_driver::ClientModule;
                module.on_capabilities_changed(&caps_impl);
            }));
        }

        /// On theme changed trampoline (#723).
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn reovim_client_module_on_theme_changed(
            module: *mut ::std::ffi::c_void,
            theme: *const ::reovim_client_driver::ffi::FfiThemeProvider,
        ) {
            let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                let module = &mut *(module as *mut #module_type);
                let theme_ref =
                    ::reovim_client_driver::ffi::FfiThemeRef::new(&*theme);
                use ::reovim_client_driver::ClientModule;
                module.on_theme_changed(&theme_ref);
            }));
        }
    };

    expanded.into()
}
