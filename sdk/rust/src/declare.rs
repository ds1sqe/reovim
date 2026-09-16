//! `declare!`: an api in the shape of 00-core.md's text form, as
//! Rust tokens, yielding what the design names: the `Decl`, the
//! record types, a trait for the Driver side and a typed wrapper for
//! the caller side.

use alloc::vec::Vec;

use reovim_kernel_interface::{Error, Handle, Hash};

/// What a caller-side wrapper calls: a `call` on a handle by hash
/// and method number, as a `Client` does.
pub trait Call {
    /// What a call can fail with; a reply that does not decode is
    /// `Error::Invalid` through it.
    type Error: From<Error>;

    /// The method `number` of `api` on `h` with `input`; its output.
    ///
    /// # Errors
    /// Whatever the call fails with.
    fn call(
        &mut self,
        h: Handle,
        api: Hash,
        number: u16,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, Self::Error>;
}

/// An api as 00-core.md writes it. One module per api holding
/// `decl()`, `hash()`, the records, `trait Api`, `dispatch` and the
/// caller's `Call`.
///
/// ```ignore
/// reovim_sdk::declare! {
///     api history ver 1 {
///         0 tree() -> Tree: READ;
///         1 goto(rev: u64) -> (): WRITE;
///     }
///     record Tree { nodes: [Node; 4096], head: u64, published: u64 }
///     record Node { id: u64, parent: u64, label: utf8(16) }
/// }
/// ```
///
/// Types: `()`, `bool`, `u8`, `u32`, `u64`, `i64`, `f64`, `Rd`,
/// `utf8(max)`, `[T; max]` (a Sequence) and a record's name. A method
/// takes no argument or one. Doc comments before `api` document the
/// module.
#[macro_export]
macro_rules! declare {
    (
        $( #[$meta:meta] )*
        api $api:ident ver $ver:literal {
            $( $number:literal $method:ident
                ( $( $arg:ident : $in:tt $( ( $inb:expr ) )? )? )
                -> $out:tt $( ( $outb:expr ) )? : $right:ident ; )*
        }
        $( record $record:ident {
            $( $field:ident : $ft:tt $( ( $fb:expr ) )? ),* $(,)?
        } )*
    ) => {
        $( #[$meta] )*
        #[allow(clippy::module_name_repetitions)]
        pub mod $api {
            use $crate::__interface::{
                Box, Calls, Decl, Error, Field, Handle, Hash, Method, Rd,
                Rights, ToString, Ty, Vec, vec,
            };
            use $crate::{Decode, Encode, Reader, Typed};

            /// The declaration, as the Kernel hashes it.
            #[must_use]
            pub fn decl() -> Decl {
                Decl {
                    api: stringify!($api).to_string(),
                    ver: $ver,
                    methods: vec![ $( Method {
                        number: $number,
                        name: stringify!($method).to_string(),
                        input: $crate::__ty!(
                            $( $in $( ( $inb ) )? )? ),
                        output: $crate::__ty!( $out $( ( $outb ) )? ),
                        right: Rights::$right,
                    } ),* ],
                }
            }

            /// Its identity.
            #[must_use]
            pub fn hash() -> Hash {
                decl().hash()
            }

            $(
                // A record may hold an `f64`, so `Eq` is not promised.
                #[allow(clippy::derive_partial_eq_without_eq)]
                #[derive(Clone, Debug, PartialEq)]
                pub struct $record {
                    $( pub $field: $crate::__rust!( $ft $( ( $fb ) )? ), )*
                }

                impl Typed for $record {
                    fn ty() -> Ty {
                        Ty::Record(vec![ $( Field {
                            name: stringify!($field).to_string(),
                            ty: $crate::__ty!( $ft $( ( $fb ) )? ),
                        } ),* ])
                    }
                }

                impl Encode for $record {
                    fn encode(&self, out: &mut Vec<u8>) {
                        $( self.$field.encode(out); )*
                    }
                }

                impl Decode for $record {
                    fn decode(r: &mut Reader<'_>) -> Option<Self> {
                        Some(Self { $( $field: Decode::decode(r)?, )* })
                    }
                }
            )*

            /// The Driver side: one method per declared method.
            pub trait Api {
                $( $crate::__method! {
                    $method ( $( $arg : $in $( ( $inb ) )? )? )
                    -> $out $( ( $outb ) )?
                } )*
            }

            /// Method `number` of `api` with `input`; its output bytes.
            ///
            /// # Errors
            /// `Invalid` when `input` is not one value of the method's
            /// input type, `Unsupported` for a number not declared,
            /// else what the method says.
            pub fn dispatch(
                api: &impl Api,
                ctx: &dyn Calls,
                rd: Rd,
                number: u16,
                input: &[u8],
            ) -> Result<Vec<u8>, Error> {
                match number {
                    $( $number => $crate::__dispatch! {
                        api ctx rd input
                        $method ( $( $arg )? )
                    }, )*
                    _ => Err(Error::Unsupported),
                }
            }

            /// The caller side: the api on one handle.
            pub struct Call<'a, C: $crate::Call>(pub &'a mut C, pub Handle);

            impl<C: $crate::Call> Call<'_, C> {
                $( $crate::__call! {
                    $number $method ( $( $arg : $in $( ( $inb ) )? )? )
                    -> $out $( ( $outb ) )?
                } )*
            }
        }
    };
}

/// The `Ty` of a DSL type.
#[doc(hidden)]
#[macro_export]
macro_rules! __ty {
    () => { Ty::Unit };
    (()) => { Ty::Unit };
    (utf8 ($max:expr)) => { Ty::Utf8($max) };
    ([$item:tt $( ( $b:expr ) )? ; $max:expr]) => {
        Ty::Sequence($max, Box::new($crate::__ty!($item $( ( $b ) )?)))
    };
    ($name:ident) => { <$name as Typed>::ty() };
}

/// The Rust type of a DSL type.
#[doc(hidden)]
#[macro_export]
macro_rules! __rust {
    () => { () };
    (()) => { () };
    (utf8 ($max:expr)) => { $crate::__interface::String };
    ([$item:tt $( ( $b:expr ) )? ; $max:expr]) => {
        $crate::__interface::Vec<$crate::__rust!($item $( ( $b ) )?)>
    };
    ($name:ident) => { $name };
}

/// One trait method: no argument for `()`, one otherwise.
#[doc(hidden)]
#[macro_export]
macro_rules! __method {
    ($method:ident () -> $out:tt $( ( $outb:expr ) )?) => {
        fn $method(
            &self,
            ctx: &dyn Calls,
            rd: Rd,
        ) -> Result<$crate::__rust!($out $( ( $outb ) )?), Error>;
    };
    ($method:ident ($arg:ident : $in:tt $( ( $inb:expr ) )?)
        -> $out:tt $( ( $outb:expr ) )?) => {
        fn $method(
            &self,
            ctx: &dyn Calls,
            rd: Rd,
            $arg: $crate::__rust!($in $( ( $inb ) )?),
        ) -> Result<$crate::__rust!($out $( ( $outb ) )?), Error>;
    };
}

/// One dispatch arm: decode the input if any, encode the answer.
#[doc(hidden)]
#[macro_export]
macro_rules! __dispatch {
    ($api:ident $ctx:ident $rd:ident $input:ident $method:ident ()) => {
        $crate::decode::<()>($input)
            .and_then(|()| $api.$method($ctx, $rd))
            .map(|out| $crate::encode(&out))
    };
    ($api:ident $ctx:ident $rd:ident $input:ident
        $method:ident ($arg:ident)) => {
        $crate::decode($input)
            .and_then(|$arg| $api.$method($ctx, $rd, $arg))
            .map(|out| $crate::encode(&out))
    };
}

/// One wrapper method: encode the argument if any, decode the answer.
#[doc(hidden)]
#[macro_export]
macro_rules! __call {
    ($number:literal $method:ident () -> $out:tt $( ( $outb:expr ) )?) => {
        /// The declared method.
        ///
        /// # Errors
        /// What the call fails with; an answer that does not decode
        /// is `Invalid`.
        pub fn $method(
            &mut self,
        ) -> Result<$crate::__rust!($out $( ( $outb ) )?), C::Error> {
            let out = self.0.call(self.1, hash(), $number, Vec::new())?;
            Ok($crate::decode(&out)?)
        }
    };
    ($number:literal $method:ident ($arg:ident : $in:tt $( ( $inb:expr ) )?)
        -> $out:tt $( ( $outb:expr ) )?) => {
        /// The declared method.
        ///
        /// # Errors
        /// What the call fails with; an answer that does not decode
        /// is `Invalid`.
        pub fn $method(
            &mut self,
            $arg: $crate::__rust!($in $( ( $inb ) )?),
        ) -> Result<$crate::__rust!($out $( ( $outb ) )?), C::Error> {
            let input = $crate::encode(&$arg);
            let out = self.0.call(self.1, hash(), $number, input)?;
            Ok($crate::decode(&out)?)
        }
    };
}
