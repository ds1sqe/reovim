#![no_std]
#![forbid(unsafe_code)]
//! Named unsigned flags with checked conversion at protocol boundaries.

/// Declares a transparent flag type. Unknown raw bits are rejected.
/// ```
/// use reovim_bitflags::rv_bitflags;
/// rv_bitflags! {
///     pub struct Access: u32 {
///         const READ = 1;
///         const WRITE = 2;
///     }
/// }
/// let access = Access::READ | Access::WRITE;
/// assert!(access.contains(Access::READ));
/// assert_eq!(Access::from_bits(4), None);
/// ```
/// Distinct flag domains cannot be combined:
/// ```compile_fail
/// use reovim_bitflags::rv_bitflags;
/// rv_bitflags! { struct Access: u32 { const READ = 1; } }
/// rv_bitflags! { struct Events: u32 { const CHANGED = 1; } }
/// let invalid = Access::READ | Events::CHANGED;
/// ```
#[macro_export]
macro_rules! rv_bitflags {
    ($(#[$meta:meta])* $vis:vis struct $name:ident: $repr:ty {
        $($(#[$flag_meta:meta])* const $flag:ident = $value:expr;)+
    }) => {
        const _: () = assert!(
            <$repr>::MIN == 0, "flags require an unsigned integer"
        );
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        $vis struct $name($repr);

        impl $name {
            $($(#[$flag_meta])* pub const $flag: Self = Self($value);)+

            #[must_use]
            pub const fn empty() -> Self { Self(0) }

            #[must_use]
            pub const fn bits(self) -> $repr { self.0 }

            #[must_use]
            pub const fn from_bits(bits: $repr) -> Option<Self> {
                let known = 0 $(| Self::$flag.0)+;
                if bits & !known == 0 { Some(Self(bits)) } else { None }
            }

            #[must_use]
            pub const fn is_empty(self) -> bool { self.0 == 0 }

            #[must_use]
            pub const fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }

            #[must_use]
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }
        }

        impl core::ops::BitOr for $name {
            type Output = Self;
            fn bitor(self, other: Self) -> Self { self.union(other) }
        }
    };
}
