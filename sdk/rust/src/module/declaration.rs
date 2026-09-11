use alloc::{boxed::Box, vec::Vec};
use reovim_kernel_interface::{Connection, Contract, abi, provider};

/// One immutable native surface. All views refer to static module data.
#[repr(transparent)]
pub struct Surface(abi::Surface);
// Construction accepts immutable static strings/slices only.
unsafe impl Sync for Surface {}

impl Surface {
    #[must_use]
    pub const fn new(
        name: &'static str,
        version: u32,
        methods: provider::Permissions,
        input: &'static [provider::Permissions],
        output: &'static [provider::Permissions],
    ) -> Self {
        Self(abi::Surface {
            contract: abi::Contract { name: view(name.as_bytes()), version },
            methods: methods.bits(),
            input: input.as_ptr().cast(),
            input_count: input.len(),
            output: output.as_ptr().cast(),
            output_count: output.len(),
        })
    }

    pub(super) fn owned(&self) -> provider::Surface {
        // Safe construction fixes every view's lifetime and element layout.
        let name = unsafe {
            core::slice::from_raw_parts(
                self.0.contract.name.data,
                self.0.contract.name.len,
            )
        };
        let input = unsafe {
            core::slice::from_raw_parts(self.0.input, self.0.input_count)
        };
        let output = unsafe {
            core::slice::from_raw_parts(self.0.output, self.0.output_count)
        };
        provider::Surface {
            contract: Contract::new(
                core::str::from_utf8(name).expect("static UTF-8 name"),
                self.0.contract.version,
            ),
            methods: permissions(self.0.methods),
            input: input.iter().map(|value| permissions(*value)).collect(),
            output: output.iter().map(|value| permissions(*value)).collect(),
        }
    }
}

const fn permissions(bits: u32) -> provider::Permissions {
    provider::Permissions::from_bits(bits).expect("safe static permissions")
}

/// An arbitrary relative Resource export; names carry no dispatch semantics.
pub struct Export {
    pub path: &'static str,
    pub surfaces: &'static [Surface],
    pub forwarding: bool,
}

impl Export {
    #[must_use]
    pub fn surfaces(&self) -> Box<[provider::Surface]> {
        self.surfaces.iter().map(Surface::owned).collect()
    }

    pub(super) const fn view(&self) -> abi::ResourceExport {
        abi::ResourceExport {
            path: view(self.path.as_bytes()),
            surfaces: self.surfaces.as_ptr().cast(),
            surface_count: self.surfaces.len(),
            forwarding: if self.forwarding { 1 } else { 0 },
        }
    }
}

/// Inert Resource construction plus passive export metadata for any module.
/// Provider order matches export order; each provider owns its private state.
pub struct Definition {
    pub exports: &'static [Export],
    pub construct: fn(Connection) -> Vec<provider::ResourceProvider>,
}

pub(super) const fn view(bytes: &'static [u8]) -> abi::Bytes {
    abi::Bytes { data: bytes.as_ptr(), len: bytes.len() }
}
