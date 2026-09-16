/// Declares an open set of Resource handlers and its matching C metadata.
///
/// State construction is inert. Async handlers borrow state retained by the
/// adapter and return `Result<Message, Error>` or preserve `RequestError`
/// through a forwarding handler; no executor is generated.
///
/// Optional `contract: ("name", schema)` and `requires: [("name", schema)]`
/// declare passive catalog metadata. They do not construct or activate any
/// provider. An unnamed declaration is not selectable by a boot contract.
/// Optional `unload: State::unload` receives shared state and returns
/// `Result<(), Error>`. It may be called for each export and on retries, so
/// successful cleanup must be idempotent. Failure retains the native instance.
#[macro_export]
macro_rules! rv_module {
    (
        $visibility:vis $name:ident {
            contract: ($identity:literal, $version:literal),
            $(requires: [$(($required:literal, $schema:literal)),* $(,)?],)?
            model: {
                $(initial_input: [$($initial_rights:expr),* $(,)?],)?
                $(references: $reference_rights:expr,)?
                initialize ($initial:literal, $initial_version:literal) =>
                    $initialize:path,
                prepare ($edit:literal, $edit_version:literal) => $prepare:path,
                observe ($read:literal, $read_version:literal) =>
                    $observe:path $(, inspect => $inspect:path)?
                $(, initialize_async => $initialize_async:path)?
                $(, prepare_async => $prepare_async:path)? $(,)?
            } $(,)?
        }
    ) => {
        #[doc = concat!(
            "Passive Model declaration; inspection does not activate it.",
            "\n```text\nlet table = ", stringify!($name), ".table();\n```\n")]
        $visibility static $name: $crate::__private::ModelDefinition =
            $crate::__private::ModelDefinition {
                identity: $crate::__private::Identity::new($identity, $version),
                dependencies: &[$($(
                    $crate::__private::Identity::new($required, $schema)
                ),*)?],
                initial: $crate::__private::Identity::new(
                    $initial, $initial_version
                ),
                initial_input: &[$($($initial_rights),*)?],
                references: $crate::rv_module!(
                    @references $($reference_rights)?
                ),
                edit: $crate::__private::Identity::new($edit, $edit_version),
                read: $crate::__private::Identity::new($read, $read_version),
                initialize: $initialize,
                prepare: $prepare,
                observe: $observe,
                inspect: $crate::rv_module!(@inspect $($inspect)?),
                initialize_async: $crate::rv_module!(
                    @inspect $($initialize_async)?
                ),
                prepare_async: $crate::rv_module!(@inspect $($prepare_async)?),
            };
    };
    (@inspect) => { None };
    (@references) => { None };
    (@references $rights:expr) => { Some($rights) };
    (@inspect $inspect:path) => { Some($inspect) };
    (
        $visibility:vis $name:ident {
            $(contract: ($identity:literal, $version:literal),)?
            $(requires: [$(($required:literal, $schema:literal)),* $(,)?],)?
            state: $construct:path,
            $(unload: $unload:path,)?
            resources: {
                $( $path:literal => {
                    $( $method:ident $contract:path => $handler:path ),+ $(,)?
                } ),+ $(,)?
            } $(,)?
        }
    ) => {
        #[doc = concat!(
            "Passive Resource declaration; inspection does not activate it.",
            "\n```text\nlet table = ", stringify!($name), ".table();\n```\n")]
        $visibility static $name: $crate::__private::Definition = {
            fn construct(kernel: $crate::Connection)
                -> $crate::__private::Vec<$crate::__private::ResourceProvider>
            {
                let state = $crate::__private::Rc::new($construct(kernel));
                let unload: Option<
                    fn(&mut $crate::__private::Rc<_>)
                        -> Result<(), $crate::Error>
                > =
                    $crate::rv_module!(@unload $($unload)?);
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
                            unload,
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
    (@unload) => { None };
    (@unload $unload:path) => {
        Some(|state: &mut $crate::__private::Rc<_>| $unload(&**state))
    };
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
        /// Borrows the module table while the library remains loaded.
        /// ```text
        /// const struct rv_module *table = rv_module_entry();
        /// ```
        pub const extern "C" fn rv_module_entry()
        -> *const $crate::__private::abi::Module {
            const TABLE: $crate::__private::abi::Module = $definition.table();
            &TABLE
        }
    };
}
