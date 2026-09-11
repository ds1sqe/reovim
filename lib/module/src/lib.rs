#![no_std]
#![deny(unsafe_code)]
//! Safe Rust declarations over the C Resource-module interface.

extern crate alloc;

pub use reovim_kernel_interface::provider::Permissions;
pub use reovim_kernel_interface::{
    Connection, Error, Message, RequestError, ResourceId,
};

/// A module-owned contract and its reference permissions.
pub struct Contract {
    name: &'static str,
    version: u32,
    input: &'static [Permissions],
    output: &'static [Permissions],
    forwarding: bool,
}

impl Contract {
    #[must_use]
    pub const fn new(name: &'static str, version: u32) -> Self {
        Self { name, version, input: &[], output: &[], forwarding: false }
    }

    #[must_use]
    pub const fn input(mut self, slots: &'static [Permissions]) -> Self {
        self.input = slots;
        self
    }

    #[must_use]
    pub const fn output(mut self, slots: &'static [Permissions]) -> Self {
        self.output = slots;
        self
    }

    #[must_use]
    pub const fn forwarding(mut self) -> Self {
        self.forwarding = true;
        self
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn surface(
        &self,
        methods: Permissions,
    ) -> reovim_sdk::module::Surface {
        reovim_sdk::module::Surface::new(
            self.name,
            self.version,
            methods,
            self.input,
            self.output,
        )
    }

    #[doc(hidden)]
    #[must_use]
    pub const fn is_forwarding(&self) -> bool {
        self.forwarding
    }

    #[doc(hidden)]
    #[must_use]
    pub fn matches(&self, input: &Message) -> bool {
        input.contract.name.as_ref() == self.name
            && input.contract.version == self.version
    }
}

#[doc(hidden)]
pub mod __private {
    pub use alloc::{boxed::Box, rc::Rc, vec, vec::Vec};
    pub use reovim_kernel_interface::{
        abi,
        provider::{ResourceOps, ResourceProvider},
    };
    pub use reovim_sdk::module::{
        Definition, Export, Identity, ModelDefinition,
    };

    /// # Panics
    /// Compile-time declarations must use one reference policy per Resource.
    #[must_use]
    pub const fn forwarding(values: &[bool]) -> bool {
        let value = values[0];
        let mut index = 1;
        while index < values.len() {
            assert!(
                values[index] == value,
                "mixed Resource reference policies"
            );
            index += 1;
        }
        value
    }
}

mod macros;
