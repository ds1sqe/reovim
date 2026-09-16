/// Instance-qualified subject identity; copying grants or retains nothing.
/// ```
/// use reovim_kernel_interface::ResourceId;
/// let id = ResourceId { epoch: [7; 16], serial: 1 };
/// assert_ne!(id, ResourceId { epoch: [8; 16], serial: 1 });
/// ```
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceId {
    /// Kernel instance identity; e.g. a restarted Kernel uses another epoch.
    pub epoch: [u8; 16],
    /// Nonzero serial, never reused; e.g. replacement allocates anew.
    pub serial: u64,
}

/// Caller-scoped token; e.g. copying does not extend opened lifetime.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct AccessId(pub u64);

reovim_bitflags::rv_bitflags! {
    /// Operation rights on opened access. Example: zero permits no operations;
    /// Runtime still checks caller ownership and liveness for a zero-right Ref.
    pub struct Access: u32 {
        const READ = 1;
        const WRITE = 2;
        const EXEC = 4;
        const ALL = Self::READ.bits() | Self::WRITE.bits() | Self::EXEC.bits();
    }
}

/// Copied structural state in the inspecting caller's namespace.
///
/// Flags grant no operations. EMPTY describes child entries, not data bytes.
/// ```
/// use reovim_kernel_interface::resource::ResourceFlags;
/// let flags = ResourceFlags::CONTAINER | ResourceFlags::EMPTY;
/// assert!(flags.contains(ResourceFlags::CONTAINER));
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceFlags(u32);

impl ResourceFlags {
    /// Supports child listing, whether empty or populated.
    /// ```
    /// use reovim_kernel_interface::resource::ResourceFlags;
    /// assert_eq!(ResourceFlags::CONTAINER.bits(), 1);
    /// ```
    pub const CONTAINER: Self = Self(1);

    /// No caller-visible children at inspection; valid with CONTAINER only.
    /// ```
    /// use reovim_kernel_interface::resource::ResourceFlags;
    /// let empty = ResourceFlags::CONTAINER | ResourceFlags::EMPTY;
    /// assert!(empty.contains(ResourceFlags::EMPTY));
    /// ```
    pub const EMPTY: Self = Self(2);

    /// Validates known bits and the EMPTY-requires-CONTAINER invariant.
    /// ```
    /// use reovim_kernel_interface::resource::ResourceFlags;
    /// assert!(ResourceFlags::from_bits(0).is_some()); // Non-container.
    /// assert!(ResourceFlags::from_bits(2).is_none()); // Invalid EMPTY alone.
    /// assert!(ResourceFlags::from_bits(4).is_none()); // Unknown flag.
    /// ```
    #[must_use]
    pub const fn from_bits(bits: u32) -> Option<Self> {
        if bits & !3 != 0 || bits == 2 { None } else { Some(Self(bits)) }
    }

    /// Encodes the copied flags for the native/service boundary.
    /// ```
    /// use reovim_kernel_interface::resource::ResourceFlags;
    /// let flags = ResourceFlags::from_bits(3).unwrap();
    /// assert_eq!(flags.bits(), 3);
    /// ```
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Tests a flag without interpreting it as access authority.
    /// ```
    /// use reovim_kernel_interface::resource::ResourceFlags;
    /// let leaf = ResourceFlags::from_bits(0).unwrap();
    /// assert!(!leaf.contains(ResourceFlags::CONTAINER));
    /// ```
    #[must_use]
    pub const fn contains(self, flags: Self) -> bool {
        self.0 & flags.0 == flags.0
    }
}

impl core::ops::BitOr for ResourceFlags {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
