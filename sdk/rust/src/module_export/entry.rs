/// Exports the native entry for one ordinary Rust module implementation.
///
/// The artifact depends on SDK and Interface. Required paths and bounds are
/// constants; inspection returns metadata without constructing an instance.
/// The explicit unsafe block acknowledges the constructor's native contract.
/// # Safety
/// The implementation's constructor must be inert, including on rejection:
/// no external effects, outstanding native views or retained context on error.
/// Native callers must retain the artifact and borrowed capabilities, serialize
/// driver calls and destroy only after Closed and the last dependent owner.
#[macro_export]
macro_rules! export_module {
    (unsafe {
        implementation: $implementation:expr,
        id: $id:expr,
        name: $name:expr,
        revision: $revision:expr,
        scopes: $scopes:expr,
        dependencies: [$($dependency:expr),* $(,)?],
        required: [$($path:expr),* $(,)?],
        export_limit: $exports:expr,
        limits: $limits:expr,
        views: $views:expr $(,)?
    }) => {
        const _: () = {
            use ::reovim_kernel_interface::abi;
            const NAME: &[u8] = $name;
            const DEPENDENCIES: &[abi::ModuleDependency] = &[$($dependency),*];
            const REQUIRED: &[abi::Bytes] = &[$({
                let path: &[u8] = $path;
                abi::Bytes { ptr: path.as_ptr(), len: path.len() }
            }),*];
            const LIMITS: ::reovim_kernel_interface::declaration::ValueLimits =
                $limits;
            const VIEWS: usize = $views;
            const DESCRIPTOR: &abi::Module = &abi::Module {
                bytes: core::mem::size_of::<abi::Module>(),
                version: abi::VERSION,
                id: $id,
                name: abi::Bytes { ptr: NAME.as_ptr(), len: NAME.len() },
                revision: $revision,
                scopes: $scopes,
                dependencies: DEPENDENCIES.as_ptr(),
                dependency_count: DEPENDENCIES.len(),
                required: REQUIRED.as_ptr(),
                required_count: REQUIRED.len(),
                export_limit: $exports,
                construct: Some(construct),
            };

            #[unsafe(no_mangle)]
            pub const extern "C" fn rv_module_entry() -> *const abi::Module {
                DESCRIPTOR
            }

            unsafe extern "C" fn construct(
                context: *const abi::ModuleContext,
                output: *mut abi::ModuleInstance,
                error: *mut abi::Error,
            ) -> u32 {
                // SAFETY: The macro's unsafe invocation promises an inert
                // constructor. Native entry callers supply valid records and
                // retain code/capabilities through the SDK-owned lifecycle.
                unsafe {
                    $crate::export_module_instance(
                        context, output, error, $implementation, LIMITS, VIEWS,
                    )
                }
            }
        };
    };
}
