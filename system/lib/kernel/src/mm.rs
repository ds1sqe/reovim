//! Memory-management bridge.
//!
//! Upper/pure crates receive `uapi/mm` control tables from composition roots.
//! This bridge backs the allocation table with the lower platform allocator
//! slots without exposing `kabi/platform` upward.

use {
    core::{
        alloc::Layout,
        cell::UnsafeCell,
        ptr::NonNull,
        sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    reovim_kabi_platform::handle,
    reovim_uapi_mm::{AllocControl, AllocError},
};

/// Maximum retained address-space records.
pub const MAX_ADDRESS_SPACE_RECORDS: usize = 64;
/// Maximum retained memory-object records backing address-space regions.
pub const MAX_ADDRESS_SPACE_OBJECT_RECORDS: usize = MAX_ADDRESS_SPACE_RECORDS * 2;
/// Maximum retained page-table records, one per address space with mapped pages.
pub const MAX_ADDRESS_SPACE_PAGE_TABLE_RECORDS: usize = MAX_ADDRESS_SPACE_RECORDS;
/// Maximum retained leaf page mapping records.
pub const MAX_ADDRESS_SPACE_PAGE_RECORDS: usize = MAX_ADDRESS_SPACE_RECORDS * 8;
/// Diagnostic page size used for initial user address-space region layout.
pub const USER_PAGE_BYTES: usize = 4096;
/// Initial user text virtual base for source-backed executable images.
pub const USER_TEXT_BASE: usize = 0x0040_0000;
/// Initial user stack top used for reserved user stack regions.
pub const USER_STACK_TOP: usize = 0x8000_0000;
/// Initial per-process user stack reservation recorded for mapped program images.
pub const DEFAULT_USER_STACK_BYTES: usize = 16 * 1024;
/// Region can be read.
pub const ADDRESS_SPACE_REGION_READ: u32 = 1 << 0;
/// Region can be written.
pub const ADDRESS_SPACE_REGION_WRITE: u32 = 1 << 1;
/// Region can be executed.
pub const ADDRESS_SPACE_REGION_EXEC: u32 = 1 << 2;
/// Region is user-visible.
pub const ADDRESS_SPACE_REGION_USER: u32 = 1 << 3;
/// Region is a stack reservation.
pub const ADDRESS_SPACE_REGION_STACK: u32 = 1 << 4;
/// Initial source text mapping permissions.
pub const ADDRESS_SPACE_TEXT_FLAGS: u32 =
    ADDRESS_SPACE_REGION_READ | ADDRESS_SPACE_REGION_EXEC | ADDRESS_SPACE_REGION_USER;
/// Initial user stack mapping permissions.
pub const ADDRESS_SPACE_STACK_FLAGS: u32 = ADDRESS_SPACE_REGION_READ
    | ADDRESS_SPACE_REGION_WRITE
    | ADDRESS_SPACE_REGION_USER
    | ADDRESS_SPACE_REGION_STACK;

const fn align_to_user_page(bytes: usize) -> usize {
    if bytes == 0 {
        return 0;
    }
    let rem = bytes % USER_PAGE_BYTES;
    if rem == 0 {
        bytes
    } else {
        let add = USER_PAGE_BYTES - rem;
        if bytes > usize::MAX - add {
            usize::MAX
        } else {
            bytes + add
        }
    }
}

const fn region_end(start: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if start > usize::MAX - len {
        usize::MAX
    } else {
        start + len
    }
}

const fn stack_start(stack_bytes: usize) -> usize {
    let len = align_to_user_page(stack_bytes);
    if len == 0 {
        0
    } else if len > USER_STACK_TOP {
        0
    } else {
        USER_STACK_TOP - len
    }
}

const fn stack_end(stack_bytes: usize) -> usize {
    if stack_bytes == 0 { 0 } else { USER_STACK_TOP }
}

const fn region_count(text_bytes: usize, stack_bytes: usize) -> usize {
    let mut count = 0usize;
    if text_bytes != 0 {
        count += 1;
    }
    if stack_bytes != 0 {
        count += 1;
    }
    count
}

const fn page_count_for_range(start: usize, end: usize) -> usize {
    if start == 0 || end <= start {
        0
    } else {
        align_to_user_page(end - start) / USER_PAGE_BYTES
    }
}

/// Executable image mapping attached to an address space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressSpaceImage {
    /// Loader-visible executable source/artifact path.
    pub source_path: &'static str,
    /// Bytes copied or interpreted as executable text for this image.
    pub text_bytes: usize,
    /// Checksum of mapped executable text bytes, or zero when no bytes were copied.
    pub text_checksum: u32,
    /// User stack reservation for this address space.
    pub stack_bytes: usize,
}

impl AddressSpaceImage {
    /// Kernel-owned image with no user stack reservation.
    #[must_use]
    pub const fn kernel(source_path: &'static str) -> Self {
        Self {
            source_path,
            text_bytes: 0,
            text_checksum: 0,
            stack_bytes: 0,
        }
    }

    /// Linked image code already resident in the OS image.
    #[must_use]
    pub const fn linked(source_path: &'static str) -> Self {
        Self {
            source_path,
            text_bytes: 0,
            text_checksum: 0,
            stack_bytes: DEFAULT_USER_STACK_BYTES,
        }
    }

    /// Source-backed executable bytes mapped into this address space.
    #[must_use]
    pub const fn source(source_path: &'static str, text_bytes: usize, text_checksum: u32) -> Self {
        Self {
            source_path,
            text_bytes,
            text_checksum,
            stack_bytes: DEFAULT_USER_STACK_BYTES,
        }
    }

    /// Metadata-only mapping used by transitional kernel-local admission paths.
    #[must_use]
    pub const fn metadata_only(source_path: &'static str) -> Self {
        Self {
            source_path,
            text_bytes: 0,
            text_checksum: 0,
            stack_bytes: DEFAULT_USER_STACK_BYTES,
        }
    }
}

/// Address-space lifecycle state retained for diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressSpaceState {
    /// Empty table slot.
    Empty,
    /// Address space is currently owned by a live process image.
    Active,
    /// Address space was replaced by exec or released by process completion.
    Retired,
}

impl AddressSpaceState {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Active => "active",
            Self::Retired => "retired",
        }
    }
}

/// Retained address-space memory object kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressSpaceObjectKind {
    /// Empty table slot.
    Empty,
    /// Executable text object.
    Text,
    /// Zero-filled stack object.
    Stack,
}

impl AddressSpaceObjectKind {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Text => "text",
            Self::Stack => "stack",
        }
    }
}

/// Retained memory-object backing type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressSpaceObjectBacking {
    /// Empty table slot.
    Empty,
    /// Source/executable bytes retained by the executable source store.
    SourceText,
    /// Anonymous zero-filled stack reservation.
    ZeroFill,
}

impl AddressSpaceObjectBacking {
    /// Stable diagnostic word.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::SourceText => "source-text",
            Self::ZeroFill => "zero-fill",
        }
    }
}

/// Retained memory object backing one address-space region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressSpaceObjectRecord {
    /// Memory-object identifier.
    pub id: usize,
    /// Owning address-space identifier.
    pub address_space_id: usize,
    /// Process currently or formerly owning the address space.
    pub owner_pid: usize,
    /// Process image generation associated with this object.
    pub image_generation: usize,
    /// Object lifecycle state.
    pub state: AddressSpaceState,
    /// Region/object kind.
    pub kind: AddressSpaceObjectKind,
    /// Backing storage type.
    pub backing: AddressSpaceObjectBacking,
    /// Loader-visible executable source/artifact path.
    pub source_path: &'static str,
    /// Virtual start covered by this object.
    pub start: usize,
    /// Virtual end covered by this object.
    pub end: usize,
    /// Logical source/reservation byte count.
    pub bytes: usize,
    /// Source checksum, or zero for anonymous objects.
    pub checksum: u32,
    /// Region permission flags.
    pub flags: u32,
    /// Number of diagnostic user pages covered by this object.
    pub page_count: usize,
}

impl AddressSpaceObjectRecord {
    /// Empty object record used for bounded snapshots.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            id: 0,
            address_space_id: 0,
            owner_pid: 0,
            image_generation: 0,
            state: AddressSpaceState::Empty,
            kind: AddressSpaceObjectKind::Empty,
            backing: AddressSpaceObjectBacking::Empty,
            source_path: "",
            start: 0,
            end: 0,
            bytes: 0,
            checksum: 0,
            flags: 0,
            page_count: 0,
        }
    }
}

/// Retained page-table root for an address space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressSpacePageTableRecord {
    /// Page-table record identifier.
    pub id: usize,
    /// Owning address-space identifier.
    pub address_space_id: usize,
    /// Process currently or formerly owning the address space.
    pub owner_pid: usize,
    /// Process image generation associated with this page table.
    pub image_generation: usize,
    /// Page-table lifecycle state.
    pub state: AddressSpaceState,
    /// Diagnostic page-table root id. This is not a physical address.
    pub root_table_id: usize,
    /// Loader-visible executable source/artifact path.
    pub source_path: &'static str,
    /// Total mapped user pages.
    pub mapped_pages: usize,
    /// Mapped executable text pages.
    pub text_pages: usize,
    /// Reserved stack pages.
    pub stack_pages: usize,
    /// Permission flags for text pages.
    pub text_flags: u32,
    /// Permission flags for stack pages.
    pub stack_flags: u32,
}

impl AddressSpacePageTableRecord {
    /// Empty page-table record used for bounded snapshots.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            id: 0,
            address_space_id: 0,
            owner_pid: 0,
            image_generation: 0,
            state: AddressSpaceState::Empty,
            root_table_id: 0,
            source_path: "",
            mapped_pages: 0,
            text_pages: 0,
            stack_pages: 0,
            text_flags: 0,
            stack_flags: 0,
        }
    }
}

/// Retained page-table leaf mapping for one user page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressSpacePageRecord {
    /// Page mapping record identifier.
    pub id: usize,
    /// Owning address-space identifier.
    pub address_space_id: usize,
    /// Owning page-table identifier.
    pub page_table_id: usize,
    /// Process currently or formerly owning the address space.
    pub owner_pid: usize,
    /// Process image generation associated with this page.
    pub image_generation: usize,
    /// Page lifecycle state.
    pub state: AddressSpaceState,
    /// Region/object kind.
    pub kind: AddressSpaceObjectKind,
    /// Backing storage type.
    pub backing: AddressSpaceObjectBacking,
    /// Loader-visible executable source/artifact path.
    pub source_path: &'static str,
    /// Page index within the address space's retained page-table view.
    pub page_index: usize,
    /// Page-aligned user virtual start.
    pub virtual_start: usize,
    /// Page-aligned user virtual end.
    pub virtual_end: usize,
    /// Logical bytes backed by this page before zero fill.
    pub bytes: usize,
    /// Offset into the logical backing object.
    pub source_offset: usize,
    /// Region permission flags.
    pub flags: u32,
}

impl AddressSpacePageRecord {
    /// Empty page record used for bounded snapshots.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            id: 0,
            address_space_id: 0,
            page_table_id: 0,
            owner_pid: 0,
            image_generation: 0,
            state: AddressSpaceState::Empty,
            kind: AddressSpaceObjectKind::Empty,
            backing: AddressSpaceObjectBacking::Empty,
            source_path: "",
            page_index: 0,
            virtual_start: 0,
            virtual_end: 0,
            bytes: 0,
            source_offset: 0,
            flags: 0,
        }
    }
}

/// Retained address-space ownership record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AddressSpaceRecord {
    /// Address-space identifier stored on the process record.
    pub id: usize,
    /// Process currently or formerly owning this address space.
    pub owner_pid: usize,
    /// Process image generation associated with this address space.
    pub image_generation: usize,
    /// Lifecycle state.
    pub state: AddressSpaceState,
    /// Process-visible executable path.
    pub program_path: &'static str,
    /// Loader/source kind for this process image.
    pub loader: &'static str,
    /// Stable executable entry name.
    pub entry_name: &'static str,
    /// Loader-visible executable source/artifact path.
    pub source_path: &'static str,
    /// Bytes copied or interpreted as executable text for this image.
    pub text_bytes: usize,
    /// Checksum of mapped executable text bytes.
    pub text_checksum: u32,
    /// User stack reservation for this address space.
    pub stack_bytes: usize,
    /// Count of retained user regions in this address space.
    pub region_count: usize,
    /// User virtual start of the mapped text region, or zero when absent.
    pub text_start: usize,
    /// User virtual end of the mapped text region, or zero when absent.
    pub text_end: usize,
    /// Permission flags for the mapped text region.
    pub text_flags: u32,
    /// User virtual start of the reserved stack region, or zero when absent.
    pub stack_start: usize,
    /// User virtual end of the reserved stack region, or zero when absent.
    pub stack_end: usize,
    /// Permission flags for the reserved stack region.
    pub stack_flags: u32,
    /// Retained page-table record id, or zero when no user pages are mapped.
    pub page_table_id: usize,
    /// Total mapped user pages.
    pub mapped_pages: usize,
    /// Mapped executable text pages.
    pub text_pages: usize,
    /// Reserved stack pages.
    pub stack_pages: usize,
}

impl AddressSpaceRecord {
    /// Empty address-space record used for bounded snapshots.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            id: 0,
            owner_pid: 0,
            image_generation: 0,
            state: AddressSpaceState::Empty,
            program_path: "",
            loader: "",
            entry_name: "",
            source_path: "",
            text_bytes: 0,
            text_checksum: 0,
            stack_bytes: 0,
            region_count: 0,
            text_start: 0,
            text_end: 0,
            text_flags: 0,
            stack_start: 0,
            stack_end: 0,
            stack_flags: 0,
            page_table_id: 0,
            mapped_pages: 0,
            text_pages: 0,
            stack_pages: 0,
        }
    }

    const fn active(
        id: usize,
        owner_pid: usize,
        image_generation: usize,
        program_path: &'static str,
        loader: &'static str,
        entry_name: &'static str,
        image: AddressSpaceImage,
    ) -> Self {
        let text_start = if image.text_bytes == 0 {
            0
        } else {
            USER_TEXT_BASE
        };
        let text_end = region_end(USER_TEXT_BASE, align_to_user_page(image.text_bytes));
        let text_flags = if image.text_bytes == 0 {
            0
        } else {
            ADDRESS_SPACE_TEXT_FLAGS
        };
        let stack_start = stack_start(image.stack_bytes);
        let stack_end = stack_end(image.stack_bytes);
        let stack_flags = if image.stack_bytes == 0 {
            0
        } else {
            ADDRESS_SPACE_STACK_FLAGS
        };
        let text_pages = page_count_for_range(text_start, text_end);
        let stack_pages = page_count_for_range(stack_start, stack_end);
        let mapped_pages = text_pages + stack_pages;
        Self {
            id,
            owner_pid,
            image_generation,
            state: AddressSpaceState::Active,
            program_path,
            loader,
            entry_name,
            source_path: image.source_path,
            text_bytes: image.text_bytes,
            text_checksum: image.text_checksum,
            stack_bytes: image.stack_bytes,
            region_count: region_count(image.text_bytes, image.stack_bytes),
            text_start,
            text_end,
            text_flags,
            stack_start,
            stack_end,
            stack_flags,
            page_table_id: if mapped_pages == 0 { 0 } else { id },
            mapped_pages,
            text_pages,
            stack_pages,
        }
    }
}

/// Empty address-space record used for bounded snapshots.
pub const EMPTY_ADDRESS_SPACE_RECORD: AddressSpaceRecord = AddressSpaceRecord::empty();
/// Empty address-space memory object record used for bounded snapshots.
pub const EMPTY_ADDRESS_SPACE_OBJECT_RECORD: AddressSpaceObjectRecord =
    AddressSpaceObjectRecord::empty();
/// Empty address-space page-table record used for bounded snapshots.
pub const EMPTY_ADDRESS_SPACE_PAGE_TABLE_RECORD: AddressSpacePageTableRecord =
    AddressSpacePageTableRecord::empty();
/// Empty address-space page record used for bounded snapshots.
pub const EMPTY_ADDRESS_SPACE_PAGE_RECORD: AddressSpacePageRecord = AddressSpacePageRecord::empty();

struct AddressSpaceTable {
    records: [AddressSpaceRecord; MAX_ADDRESS_SPACE_RECORDS],
    objects: [AddressSpaceObjectRecord; MAX_ADDRESS_SPACE_OBJECT_RECORDS],
    page_tables: [AddressSpacePageTableRecord; MAX_ADDRESS_SPACE_PAGE_TABLE_RECORDS],
    pages: [AddressSpacePageRecord; MAX_ADDRESS_SPACE_PAGE_RECORDS],
    next_object_id: usize,
    next_page_id: usize,
}

impl AddressSpaceTable {
    const fn new() -> Self {
        let mut records = [AddressSpaceRecord::empty(); MAX_ADDRESS_SPACE_RECORDS];
        let page_tables =
            [AddressSpacePageTableRecord::empty(); MAX_ADDRESS_SPACE_PAGE_TABLE_RECORDS];
        records[0] = AddressSpaceRecord::active(
            1,
            1,
            1,
            "rootd",
            "kernel",
            "rootd_main",
            AddressSpaceImage::kernel("rootd"),
        );
        records[1] = AddressSpaceRecord::active(
            2,
            2,
            1,
            "root-shell",
            "kernel",
            "root_shell",
            AddressSpaceImage::kernel("root-shell"),
        );
        Self {
            records,
            objects: [AddressSpaceObjectRecord::empty(); MAX_ADDRESS_SPACE_OBJECT_RECORDS],
            page_tables,
            pages: [AddressSpacePageRecord::empty(); MAX_ADDRESS_SPACE_PAGE_RECORDS],
            next_object_id: 1,
            next_page_id: 1,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }

    fn write_active(&mut self, record: AddressSpaceRecord) -> bool {
        if !self.has_address_space_slot(record.id) {
            return false;
        }
        if !self.has_object_slots_for(record.id, record.region_count) {
            return false;
        }
        if !self.has_page_table_slot(record.id, record.page_table_id != 0) {
            return false;
        }
        if !self.has_page_slots_for(record.id, record.mapped_pages) {
            return false;
        }

        self.retire_objects(record.id);
        self.retire_page_table(record.id);
        self.retire_pages(record.id);

        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].id == record.id {
                self.records[index] = record;
                self.write_objects(record);
                self.write_page_table(record);
                self.write_pages(record);
                return true;
            }
            index += 1;
        }

        index = 0;
        while index < self.records.len() {
            if self.records[index].state == AddressSpaceState::Empty {
                self.records[index] = record;
                self.write_objects(record);
                self.write_page_table(record);
                self.write_pages(record);
                return true;
            }
            index += 1;
        }

        index = 0;
        while index < self.records.len() {
            if self.records[index].state == AddressSpaceState::Retired {
                self.records[index] = record;
                self.write_objects(record);
                self.write_page_table(record);
                self.write_pages(record);
                return true;
            }
            index += 1;
        }
        false
    }

    fn has_address_space_slot(&self, id: usize) -> bool {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].id == id
                || self.records[index].state == AddressSpaceState::Empty
                || self.records[index].state == AddressSpaceState::Retired
            {
                return true;
            }
            index += 1;
        }
        false
    }

    fn has_object_slots_for(&self, address_space_id: usize, needed: usize) -> bool {
        let mut available = 0usize;
        let mut index = 0usize;
        while index < self.objects.len() {
            let object = self.objects[index];
            if object.state == AddressSpaceState::Empty
                || object.state == AddressSpaceState::Retired
                || object.address_space_id == address_space_id
            {
                available += 1;
                if available >= needed {
                    return true;
                }
            }
            index += 1;
        }
        needed == 0
    }

    fn has_page_table_slot(&self, address_space_id: usize, needed: bool) -> bool {
        if !needed {
            return true;
        }
        let mut index = 0usize;
        while index < self.page_tables.len() {
            let page_table = self.page_tables[index];
            if page_table.state == AddressSpaceState::Empty
                || page_table.state == AddressSpaceState::Retired
                || page_table.address_space_id == address_space_id
            {
                return true;
            }
            index += 1;
        }
        false
    }

    fn has_page_slots_for(&self, address_space_id: usize, needed: usize) -> bool {
        let mut available = 0usize;
        let mut index = 0usize;
        while index < self.pages.len() {
            let page = self.pages[index];
            if page.state == AddressSpaceState::Empty
                || page.state == AddressSpaceState::Retired
                || page.address_space_id == address_space_id
            {
                available += 1;
                if available >= needed {
                    return true;
                }
            }
            index += 1;
        }
        needed == 0
    }

    fn next_object_id(&mut self) -> usize {
        let id = self.next_object_id;
        self.next_object_id = self.next_object_id.saturating_add(1);
        id
    }

    fn next_page_id(&mut self) -> usize {
        let id = self.next_page_id;
        self.next_page_id = self.next_page_id.saturating_add(1);
        id
    }

    fn write_objects(&mut self, record: AddressSpaceRecord) {
        if record.text_start != 0 && record.text_end != 0 {
            let object = AddressSpaceObjectRecord {
                id: self.next_object_id(),
                address_space_id: record.id,
                owner_pid: record.owner_pid,
                image_generation: record.image_generation,
                state: AddressSpaceState::Active,
                kind: AddressSpaceObjectKind::Text,
                backing: AddressSpaceObjectBacking::SourceText,
                source_path: record.source_path,
                start: record.text_start,
                end: record.text_end,
                bytes: record.text_bytes,
                checksum: record.text_checksum,
                flags: record.text_flags,
                page_count: record.text_pages,
            };
            let _ = self.write_object(object);
        }

        if record.stack_start != 0 && record.stack_end != 0 {
            let object = AddressSpaceObjectRecord {
                id: self.next_object_id(),
                address_space_id: record.id,
                owner_pid: record.owner_pid,
                image_generation: record.image_generation,
                state: AddressSpaceState::Active,
                kind: AddressSpaceObjectKind::Stack,
                backing: AddressSpaceObjectBacking::ZeroFill,
                source_path: record.source_path,
                start: record.stack_start,
                end: record.stack_end,
                bytes: record.stack_bytes,
                checksum: 0,
                flags: record.stack_flags,
                page_count: record.stack_pages,
            };
            let _ = self.write_object(object);
        }
    }

    fn write_page_table(&mut self, record: AddressSpaceRecord) {
        if record.page_table_id == 0 {
            return;
        }
        let page_table = AddressSpacePageTableRecord {
            id: record.page_table_id,
            address_space_id: record.id,
            owner_pid: record.owner_pid,
            image_generation: record.image_generation,
            state: AddressSpaceState::Active,
            root_table_id: record.page_table_id,
            source_path: record.source_path,
            mapped_pages: record.mapped_pages,
            text_pages: record.text_pages,
            stack_pages: record.stack_pages,
            text_flags: record.text_flags,
            stack_flags: record.stack_flags,
        };
        let _ = self.write_page_table_record(page_table);
    }

    fn write_pages(&mut self, record: AddressSpaceRecord) {
        if record.page_table_id == 0 {
            return;
        }
        let mut page_index = 0usize;
        page_index = self.write_region_pages(
            record,
            page_index,
            AddressSpaceObjectKind::Text,
            AddressSpaceObjectBacking::SourceText,
            record.text_start,
            record.text_pages,
            record.text_bytes,
            record.text_flags,
        );
        let _ = self.write_region_pages(
            record,
            page_index,
            AddressSpaceObjectKind::Stack,
            AddressSpaceObjectBacking::ZeroFill,
            record.stack_start,
            record.stack_pages,
            record.stack_bytes,
            record.stack_flags,
        );
    }

    fn write_region_pages(
        &mut self,
        record: AddressSpaceRecord,
        mut page_index: usize,
        kind: AddressSpaceObjectKind,
        backing: AddressSpaceObjectBacking,
        start: usize,
        pages: usize,
        logical_bytes: usize,
        flags: u32,
    ) -> usize {
        let mut index = 0usize;
        while index < pages {
            let offset = index * USER_PAGE_BYTES;
            let bytes = if logical_bytes > offset {
                let remaining = logical_bytes - offset;
                if remaining > USER_PAGE_BYTES {
                    USER_PAGE_BYTES
                } else {
                    remaining
                }
            } else {
                0
            };
            let virtual_start = start + offset;
            let page = AddressSpacePageRecord {
                id: self.next_page_id(),
                address_space_id: record.id,
                page_table_id: record.page_table_id,
                owner_pid: record.owner_pid,
                image_generation: record.image_generation,
                state: AddressSpaceState::Active,
                kind,
                backing,
                source_path: record.source_path,
                page_index,
                virtual_start,
                virtual_end: virtual_start + USER_PAGE_BYTES,
                bytes,
                source_offset: offset,
                flags,
            };
            let _ = self.write_page_record(page);
            page_index += 1;
            index += 1;
        }
        page_index
    }

    fn write_page_record(&mut self, page: AddressSpacePageRecord) -> bool {
        let mut index = 0usize;
        while index < self.pages.len() {
            if self.pages[index].state == AddressSpaceState::Empty {
                self.pages[index] = page;
                return true;
            }
            index += 1;
        }

        index = 0;
        while index < self.pages.len() {
            if self.pages[index].state == AddressSpaceState::Retired {
                self.pages[index] = page;
                return true;
            }
            index += 1;
        }
        false
    }

    fn write_page_table_record(&mut self, page_table: AddressSpacePageTableRecord) -> bool {
        let mut index = 0usize;
        while index < self.page_tables.len() {
            if self.page_tables[index].state == AddressSpaceState::Empty {
                self.page_tables[index] = page_table;
                return true;
            }
            index += 1;
        }

        index = 0;
        while index < self.page_tables.len() {
            if self.page_tables[index].state == AddressSpaceState::Retired {
                self.page_tables[index] = page_table;
                return true;
            }
            index += 1;
        }
        false
    }

    fn write_object(&mut self, object: AddressSpaceObjectRecord) -> bool {
        let mut index = 0usize;
        while index < self.objects.len() {
            if self.objects[index].state == AddressSpaceState::Empty {
                self.objects[index] = object;
                return true;
            }
            index += 1;
        }

        index = 0;
        while index < self.objects.len() {
            if self.objects[index].state == AddressSpaceState::Retired {
                self.objects[index] = object;
                return true;
            }
            index += 1;
        }
        false
    }

    fn retire(&mut self, owner_pid: usize, id: usize) -> bool {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].id == id && self.records[index].owner_pid == owner_pid {
                if self.records[index].state == AddressSpaceState::Active {
                    self.records[index].state = AddressSpaceState::Retired;
                }
                self.retire_objects(id);
                self.retire_page_table(id);
                self.retire_pages(id);
                return true;
            }
            index += 1;
        }
        false
    }

    fn retire_objects(&mut self, address_space_id: usize) {
        let mut index = 0usize;
        while index < self.objects.len() {
            if self.objects[index].address_space_id == address_space_id
                && self.objects[index].state == AddressSpaceState::Active
            {
                self.objects[index].state = AddressSpaceState::Retired;
            }
            index += 1;
        }
    }

    fn retire_page_table(&mut self, address_space_id: usize) {
        let mut index = 0usize;
        while index < self.page_tables.len() {
            if self.page_tables[index].address_space_id == address_space_id
                && self.page_tables[index].state == AddressSpaceState::Active
            {
                self.page_tables[index].state = AddressSpaceState::Retired;
            }
            index += 1;
        }
    }

    fn retire_pages(&mut self, address_space_id: usize) {
        let mut index = 0usize;
        while index < self.pages.len() {
            if self.pages[index].address_space_id == address_space_id
                && self.pages[index].state == AddressSpaceState::Active
            {
                self.pages[index].state = AddressSpaceState::Retired;
            }
            index += 1;
        }
    }

    fn snapshot(&self, out: &mut [AddressSpaceRecord]) -> usize {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < self.records.len() && written < out.len() {
            if self.records[index].state != AddressSpaceState::Empty {
                out[written] = self.records[index];
                written += 1;
            }
            index += 1;
        }
        written
    }

    fn find(&self, id: usize) -> Option<AddressSpaceRecord> {
        let mut index = 0usize;
        while index < self.records.len() {
            if self.records[index].id == id && self.records[index].state != AddressSpaceState::Empty
            {
                return Some(self.records[index]);
            }
            index += 1;
        }
        None
    }

    fn snapshot_objects(&self, out: &mut [AddressSpaceObjectRecord]) -> usize {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < self.objects.len() && written < out.len() {
            if self.objects[index].state != AddressSpaceState::Empty {
                out[written] = self.objects[index];
                written += 1;
            }
            index += 1;
        }
        written
    }

    fn snapshot_page_tables(&self, out: &mut [AddressSpacePageTableRecord]) -> usize {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < self.page_tables.len() && written < out.len() {
            if self.page_tables[index].state != AddressSpaceState::Empty {
                out[written] = self.page_tables[index];
                written += 1;
            }
            index += 1;
        }
        written
    }

    fn snapshot_pages(&self, out: &mut [AddressSpacePageRecord]) -> usize {
        let mut written = 0usize;
        let mut index = 0usize;
        while index < self.pages.len() && written < out.len() {
            if self.pages[index].state != AddressSpaceState::Empty {
                out[written] = self.pages[index];
                written += 1;
            }
            index += 1;
        }
        written
    }
}

struct AddressSpaceCell(UnsafeCell<AddressSpaceTable>);

// SAFETY: mutable access is serialized by `ADDRESS_SPACE_LOCK`.
unsafe impl Sync for AddressSpaceCell {}

static ADDRESS_SPACES: AddressSpaceCell =
    AddressSpaceCell(UnsafeCell::new(AddressSpaceTable::new()));
static ADDRESS_SPACE_LOCK: AtomicBool = AtomicBool::new(false);
static NEXT_ADDRESS_SPACE_ID: AtomicUsize = AtomicUsize::new(3);

struct AddressSpaceGuard;

impl AddressSpaceGuard {
    fn acquire() -> Self {
        while ADDRESS_SPACE_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for AddressSpaceGuard {
    fn drop(&mut self) {
        ADDRESS_SPACE_LOCK.store(false, Ordering::Release);
    }
}

fn with_address_spaces<R>(f: impl FnOnce(&mut AddressSpaceTable) -> R) -> R {
    let _guard = AddressSpaceGuard::acquire();
    // SAFETY: `ADDRESS_SPACE_LOCK` serializes access to the table.
    let table = unsafe { &mut *ADDRESS_SPACES.0.get() };
    f(table)
}

/// Resets retained address-space ownership to rootd plus root shell.
pub fn reset_address_spaces() {
    NEXT_ADDRESS_SPACE_ID.store(3, Ordering::Release);
    with_address_spaces(AddressSpaceTable::reset);
}

/// Rebinds the reserved shell-session address-space record to the active shell image.
pub fn install_shell_address_space(
    program_path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
) {
    let record = AddressSpaceRecord::active(
        2,
        2,
        1,
        program_path,
        loader,
        entry_name,
        AddressSpaceImage::linked(program_path),
    );
    let _ = with_address_spaces(|table| table.write_active(record));
}

/// Allocates and records a fresh process address space.
#[must_use]
pub fn allocate_process_address_space(
    owner_pid: usize,
    image_generation: usize,
    program_path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    image: AddressSpaceImage,
) -> Option<usize> {
    let id = NEXT_ADDRESS_SPACE_ID.fetch_add(1, Ordering::AcqRel);
    let record = AddressSpaceRecord::active(
        id,
        owner_pid,
        image_generation,
        program_path,
        loader,
        entry_name,
        image,
    );
    if with_address_spaces(|table| table.write_active(record)) {
        Some(id)
    } else {
        None
    }
}

/// Retires the current address space and allocates a replacement for same-PID exec.
#[must_use]
pub fn replace_process_address_space(
    owner_pid: usize,
    current_id: usize,
    next_image_generation: usize,
    program_path: &'static str,
    loader: &'static str,
    entry_name: &'static str,
    image: AddressSpaceImage,
) -> Option<usize> {
    let id = NEXT_ADDRESS_SPACE_ID.fetch_add(1, Ordering::AcqRel);
    let record = AddressSpaceRecord::active(
        id,
        owner_pid,
        next_image_generation,
        program_path,
        loader,
        entry_name,
        image,
    );
    if with_address_spaces(|table| {
        table.retire(owner_pid, current_id);
        table.write_active(record)
    }) {
        Some(id)
    } else {
        None
    }
}

/// Marks an address space retired after process completion.
pub fn retire_process_address_space(owner_pid: usize, id: usize) {
    let _ = with_address_spaces(|table| table.retire(owner_pid, id));
}

/// Returns one retained address-space record by id.
#[must_use]
pub fn address_space(id: usize) -> Option<AddressSpaceRecord> {
    with_address_spaces(|table| table.find(id))
}

/// Copies retained address-space records into `out`.
pub fn snapshot_address_spaces(out: &mut [AddressSpaceRecord]) -> usize {
    with_address_spaces(|table| table.snapshot(out))
}

/// Copies retained address-space memory object records into `out`.
pub fn snapshot_address_space_objects(out: &mut [AddressSpaceObjectRecord]) -> usize {
    with_address_spaces(|table| table.snapshot_objects(out))
}

/// Copies retained address-space page-table records into `out`.
pub fn snapshot_address_space_page_tables(out: &mut [AddressSpacePageTableRecord]) -> usize {
    with_address_spaces(|table| table.snapshot_page_tables(out))
}

/// Copies retained address-space leaf page records into `out`.
pub fn snapshot_address_space_pages(out: &mut [AddressSpacePageRecord]) -> usize {
    with_address_spaces(|table| table.snapshot_pages(out))
}

/// Returns the up-face allocation control table backed by this bridge.
///
/// ```rust,no_run
/// use reovim_system_kernel::mm::alloc_control;
///
/// let _control = alloc_control();
/// ```
#[must_use]
pub const fn alloc_control() -> AllocControl {
    AllocControl::new(alloc, dealloc)
}

/// Installs this bridge's allocation control table into `lib/ds`.
///
/// # Errors
///
/// Returns [`reovim_lib_ds::alloc_backend::AllocBackendInstallError`] when the
/// process already installed an allocation backend.
pub fn install_lib_ds_alloc_backend()
-> Result<(), reovim_lib_ds::alloc_backend::AllocBackendInstallError> {
    reovim_lib_ds::alloc_backend::install(alloc_control())
}

fn alloc(layout: Layout) -> Result<NonNull<u8>, AllocError> {
    handle().alloc(layout).map_err(|_| AllocError)
}

fn dealloc(ptr: NonNull<u8>, layout: Layout) {
    handle().dealloc(ptr, layout);
}

#[cfg(feature = "selftest")]
#[path = "mm_tests.rs"]
mod mm_tests;
