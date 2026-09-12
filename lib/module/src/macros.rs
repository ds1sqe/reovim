/// Declares an open set of Resource handlers and its matching C metadata.
///
/// State construction is inert. Async handlers borrow state retained by the
/// adapter and return `Result<Message, Error>` or preserve `RequestError`
/// through a forwarding handler; no executor is generated.
///
/// Optional `contract: ("name", schema)` and `requires: [("name", schema)]`
/// declare passive catalog metadata. They do not construct or activate any
/// provider. An unnamed declaration is not selectable by a boot contract.
#[macro_export]
macro_rules! rv_module {
    (
        $visibility:vis $name:ident {
            contract: ($identity:literal, $version:literal),
            $(requires: [$(($required:literal, $schema:literal)),* $(,)?],)?
            model: {
                initialize ($initial:literal, $initial_version:literal) =>
                    $initialize:path,
                prepare ($edit:literal, $edit_version:literal) => $prepare:path,
                observe ($read:literal, $read_version:literal) =>
                    $observe:path $(, inspect => $inspect:path)? $(,)?
            } $(,)?
        }
    ) => {
        $visibility static $name: $crate::__private::ModelDefinition =
            $crate::__private::ModelDefinition {
                identity: $crate::__private::Identity::new($identity, $version),
                dependencies: &[$($(
                    $crate::__private::Identity::new($required, $schema)
                ),*)?],
                initial: $crate::__private::Identity::new(
                    $initial, $initial_version
                ),
                edit: $crate::__private::Identity::new($edit, $edit_version),
                read: $crate::__private::Identity::new($read, $read_version),
                initialize: $initialize,
                prepare: $prepare,
                observe: $observe,
                inspect: $crate::rv_module!(@inspect $($inspect)?),
            };
    };
    (@inspect) => { None };
    (@inspect $inspect:path) => { Some($inspect) };
    (
        $visibility:vis $name:ident {
            $(contract: ($identity:literal, $version:literal),)?
            $(requires: [$(($required:literal, $schema:literal)),* $(,)?],)?
            state: $construct:path,
            resources: {
                $( $path:literal => {
                    $( $method:ident $contract:path => $handler:path ),+ $(,)?
                } ),+ $(,)?
            } $(,)?
        }
    ) => {
        $visibility static $name: $crate::__private::Definition = {
            fn construct(kernel: $crate::Connection)
                -> $crate::__private::Vec<$crate::__private::ResourceProvider>
            {
                let state = $crate::__private::Rc::new($construct(kernel));
                $crate::__private::vec![$({
                    const EXPORT: $crate::__private::Export =
                    $crate::__private::Export {
                        path: $path,
                        surfaces: &[$($contract.surface(
                            $crate::rv_module!(@method $method)
                        )),+],
                        forwarding: $crate::__private::forwarding(
                            &[$($contract.is_forwarding()),+]
                        ),
                    };
                    let mut provider = $crate::__private::ResourceProvider::new(
                        state.clone(),
                        $crate::__private::ResourceOps {
                            unload: None,
                            read: |state, subject, input| {
                                $crate::rv_module!(@dispatch read, state,
                                    subject, input;
                                    $($method $contract => $handler),+)
                            },
                            write: |state, subject, input| {
                                $crate::rv_module!(@dispatch write, state,
                                    subject, input;
                                    $($method $contract => $handler),+)
                            },
                        },
                        EXPORT.surfaces(),
                        $crate::__private::Rc::new(()),
                    );
                    provider.forwarding = EXPORT.forwarding;
                    provider
                }),+]
            }
            $crate::__private::Definition {
                identity: $crate::rv_module!(@identity $($identity, $version)?),
                dependencies: &[$($(
                    $crate::__private::Identity::new($required, $schema)
                ),*)?],
                construct,
                exports: &[$($crate::__private::Export {
                    path: $path,
                    surfaces: &[$($contract.surface(
                        $crate::rv_module!(@method $method)
                    )),+],
                    forwarding: $crate::__private::forwarding(
                        &[$($contract.is_forwarding()),+]
                    ),
                }),+],
            }
        };
    };
    (@identity) => { $crate::__private::Identity::UNNAMED };
    (@identity $name:literal, $version:literal) => {
        $crate::__private::Identity::new($name, $version)
    };
    (@method read) => { $crate::Permissions::READ };
    (@method write) => { $crate::Permissions::WRITE };
    (@dispatch $wanted:ident, $state:ident, $subject:ident, $input:ident;
        $($method:ident $contract:path => $handler:path),+
    ) => {{
        $(
            if $crate::rv_module!(@method $wanted)
                == $crate::rv_module!(@method $method)
                && $contract.matches(&$input)
            {
                let state = $state.clone();
                return $crate::__private::Box::pin(async move {
                    $handler(&state, $subject, $input).await
                        .map_err($crate::RequestError::from)
                });
            }
        )+
        $crate::__private::Box::pin(async {
            Err($crate::RequestError::Failed($crate::Error::UNSUPPORTED))
        })
    }};
}

/// Exports a declaration from a native artifact; its portable crate has no
/// global entry symbol. The ABI attribute belongs to this shared adapter.
#[macro_export]
macro_rules! rv_module_entry {
    ($definition:path) => {
        #[allow(unsafe_code)]
        #[unsafe(no_mangle)]
        pub const extern "C" fn rv_module_entry()
        -> *const $crate::__private::abi::Module {
            const TABLE: $crate::__private::abi::Module = $definition.table();
            &TABLE
        }
    };
}
