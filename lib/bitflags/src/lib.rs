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
/// assert_eq!(!Access::READ, Access::WRITE);
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
            pub const fn all() -> Self { Self(0 $(| Self::$flag.0)+) }

            #[must_use]
            pub const fn bits(self) -> $repr { self.0 }

            #[must_use]
            pub const fn from_bits(bits: $repr) -> Option<Self> {
                if bits & !Self::all().0 == 0 { Some(Self(bits)) } else { None }
            }

            #[must_use]
            pub const fn is_empty(self) -> bool { self.0 == 0 }

            #[must_use]
            pub const fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }

            #[must_use]
            pub const fn intersects(self, other: Self) -> bool {
                self.0 & other.0 != 0
            }

            #[must_use]
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }

            #[must_use]
            pub const fn intersection(self, other: Self) -> Self {
                Self(self.0 & other.0)
            }

            #[must_use]
            pub const fn difference(self, other: Self) -> Self {
                Self(self.0 & !other.0)
            }
        }

        impl core::ops::BitOr for $name {
            type Output = Self;
            fn bitor(self, other: Self) -> Self { self.union(other) }
        }

        impl core::ops::BitAnd for $name {
            type Output = Self;
            fn bitand(self, other: Self) -> Self { self.intersection(other) }
        }

        impl core::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, other: Self) {
                *self = self.union(other);
            }
        }

        impl core::ops::BitAndAssign for $name {
            fn bitand_assign(&mut self, other: Self) {
                *self = self.intersection(other);
            }
        }

        impl core::ops::Not for $name {
            type Output = Self;
            fn not(self) -> Self { Self::all().difference(self) }
        }
    };
}
