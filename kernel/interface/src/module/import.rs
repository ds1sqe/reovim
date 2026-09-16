use crate::{Error, declaration::Declaration};
use alloc::{boxed::Box, string::String, vec::Vec};

/// One requested dependency export, named locally in the consumer's imports.
/// Metadata requests access; it does not authorize that access.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Import {
    name: Box<str>,
    export: Box<[u8]>,
    declaration: Declaration,
    access: u32,
    create_access: u32,
    retain_loans: bool,
}
impl Import {
    /// Captures an exact export and complete API under a local ASCII name.
    /// An empty export path selects the dependency's namespace root export.
    /// # Errors
    /// Invalid names, relative paths, masks or allocation pressure reject.
    pub fn new(
        name: &str,
        export: &[u8],
        declaration: Declaration,
        access: u32,
        create_access: u32,
        retain_loans: bool,
    ) -> Result<Self, Error> {
        if name.len() > 255 || export.len() > 4096 {
            return Err(Error::PRESSURE);
        }
        if name.is_empty()
            || !name.is_ascii()
            || name.bytes().any(|byte| byte == 0 || byte == b'/')
            || matches!(name, "." | "..")
            || (access | create_access) & !7 != 0
        {
            return Err(Error::INVALID);
        }
        if !export.is_empty()
            && export.split(|&byte| byte == b'/').any(|part| {
                part.is_empty()
                    || part == b"."
                    || part == b".."
                    || part.contains(&0)
            })
        {
            return Err(Error::INVALID);
        }
        let mut owned_name = String::new();
        owned_name
            .try_reserve_exact(name.len())
            .map_err(|_| Error::PRESSURE)?;
        owned_name.push_str(name);
        let mut owned_export = Vec::new();
        owned_export
            .try_reserve_exact(export.len())
            .map_err(|_| Error::PRESSURE)?;
        owned_export.extend_from_slice(export);
        Ok(Self {
            name: owned_name.into_boxed_str(),
            export: owned_export.into_boxed_slice(),
            declaration,
            access,
            create_access,
            retain_loans,
        })
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub fn export(&self) -> &[u8] {
        &self.export
    }
    #[must_use]
    pub const fn declaration(&self) -> &Declaration {
        &self.declaration
    }
    #[must_use]
    pub const fn access(&self) -> u32 {
        self.access
    }
    #[must_use]
    pub const fn create_access(&self) -> u32 {
        self.create_access
    }
    #[must_use]
    pub const fn retain_loans(&self) -> bool {
        self.retain_loans
    }
}
