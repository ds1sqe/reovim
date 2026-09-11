#![no_std]
#![forbid(unsafe_code)]
//! C-layout operation tables with an explicit version and extent header.

/// Declares a table or initializes its common ABI header.
/// ```
/// use reovim_vtable::rv_vtable;
/// #[repr(C)]
/// #[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// struct Version { major: u32, minor: u32, patch: u32, reserved: u32 }
/// const VERSION: Version =
///     Version { major: 0, minor: 1, patch: 0, reserved: 0 };
/// rv_vtable! {
///     struct Ops {
///         version: Version = VERSION;
///         pub read: extern "C" fn(u32) -> u32,
///     }
/// }
/// extern "C" fn read(value: u32) -> u32 { value + 1 }
/// let ops = rv_vtable!(Ops { read });
/// assert_eq!((ops.read)(4), 5);
/// assert_eq!(ops.version, Ops::ABI_VERSION);
/// assert_eq!(ops.size as usize, core::mem::size_of::<Ops>());
/// ```
#[macro_export]
macro_rules! rv_vtable {
    ($(#[$meta:meta])* $vis:vis struct $name:ident {
        version: $version_ty:ty = $version:expr;
        $($(#[$field_meta:meta])* $field_vis:vis $field:ident: $ty:ty),* $(,)?
    }) => {
        $(#[$meta])*
        #[repr(C)]
        $vis struct $name {
            pub version: $version_ty,
            pub size: u32,
            $($(#[$field_meta])* $field_vis $field: $ty),*
        }
        impl $name {
            pub const ABI_VERSION: $version_ty = $version;
            #[allow(clippy::cast_possible_truncation)]
            pub const ABI_SIZE: u32 = {
                assert!(core::mem::size_of::<Self>() <= u32::MAX as usize);
                core::mem::size_of::<Self>() as u32
            };
        }
    };
    ($table:path { $($field:ident $( : $value:expr )?),* $(,)? }) => {{
        type Table = $table;
        Table {
            version: Table::ABI_VERSION,
            size: Table::ABI_SIZE,
            $($field: $crate::rv_vtable!(@value $field $( : $value )?)),*
        }
    }};
    (@value $field:ident : $value:expr) => { $value };
    (@value $field:ident) => { $field };
}
