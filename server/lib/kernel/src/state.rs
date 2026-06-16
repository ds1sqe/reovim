//! Kernel-owned Phase 3 state substrate (§3.2, §3.3, §3.4, §4.3).
//!
//! The contract tier defines portable value shapes. This module owns kernel
//! storage and mutation mechanics for byte buffers, register carriers,
//! view-slot rows, and service leases.

use {
    reovim_lib_ds::{Bytes, Map, Seq},
    reovim_subsys_domain::{
        carrier::{CarrierStatus, CursorCarrier, PositionCarrier},
        id::{
            BufferId, CdylibId, ClientId, DomainId, RegisterId, ServiceKey, ServiceLeaseId,
            SessionId, StreamId, WindowId,
        },
        routing::RegistrationPhase,
        service::{
            ServiceAccessError, ServiceBorrow, ServiceDescriptorMeta, ServiceLease, ServiceRowState,
        },
        state::{
            EditOrigin, EditRecord, RegisterKey, RegisterScope, UndoGroup, UndoGroupId, UndoStack,
            ViewSlotFlags, ViewSlotKey,
        },
        stream::{
            BackpressureCounters, StreamControlClass, StreamControlOp, StreamHandle, StreamScheme,
            StreamState,
        },
    },
};

/// Carrier and codec limits enforced by the state substrate.
///
/// ```rust
/// use reovim_kernel::state::CarrierKernelLimits;
///
/// let limits = CarrierKernelLimits {
///     max_position_carrier_bytes: 1,
///     ..CarrierKernelLimits::default()
/// };
/// assert_eq!(limits.max_position_carrier_bytes, 1);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CarrierKernelLimits {
    /// Maximum bytes accepted in one position carrier.
    pub max_position_carrier_bytes: usize,
    /// Maximum bytes accepted in one cursor carrier.
    pub max_cursor_carrier_bytes: usize,
    /// Maximum opaque carrier bytes retained per session.
    pub max_opaque_carrier_bytes_per_session: usize,
    /// Maximum deferred restore carriers retained before a codec appears.
    pub max_deferred_restore_carriers: usize,
    /// Maximum codec rows accepted per Domain.
    pub max_codecs_per_domain: usize,
    /// Maximum `DomainTable` entries per attach.
    pub max_domain_table_entries: usize,
    /// Maximum server-rendered codec display bytes.
    pub max_codec_display_bytes: usize,
    /// Maximum invalid-carrier DS12 events per source/minute.
    pub max_invalid_carrier_per_min: u32,
}

impl CarrierKernelLimits {
    /// Default carrier and codec limits.
    const DEFAULT: Self = Self {
        max_position_carrier_bytes: 4096,
        max_cursor_carrier_bytes: 4096,
        max_opaque_carrier_bytes_per_session: 1024 * 1024,
        max_deferred_restore_carriers: 1024,
        max_codecs_per_domain: 256,
        max_domain_table_entries: 1024,
        max_codec_display_bytes: 4096,
        max_invalid_carrier_per_min: 64,
    };
}

impl Default for CarrierKernelLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Resource limits enforced by the Phase 3 in-kernel state substrate.
///
/// ```rust
/// use reovim_kernel::state::KernelStateLimits;
///
/// let limits = KernelStateLimits::default();
/// assert!(limits.max_view_slot_bytes > 0);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KernelStateLimits {
    /// Maximum opaque bytes retained in one view slot.
    pub max_view_slot_bytes: usize,
    /// Maximum window-scoped slots per `(client, buffer, window)`.
    pub max_slots_per_window: usize,
    /// Kernel-wide service lease timeout.
    pub service_lease_timeout_ms: u64,
    /// Maximum bytes stored in one register carrier.
    pub max_register_carrier_bytes: usize,
    /// Maximum client-scoped register rows per client.
    pub max_client_registers: usize,
    /// Maximum session-scoped register rows per session.
    pub max_session_registers: usize,
    /// Maximum system-scoped register rows.
    pub max_system_registers: usize,
    /// Maximum undo groups retained per buffer.
    pub max_undo_groups_per_buffer: usize,
    /// Maximum edits retained in one undo group before auto-close.
    pub max_edits_per_group: usize,
    /// Maximum undo/redo record bytes retained per buffer.
    pub max_undo_bytes_per_buffer: usize,
    /// Maximum bytes accepted in one position carrier.
    pub max_position_carrier_bytes: usize,
    /// Maximum bytes accepted in one cursor carrier.
    pub max_cursor_carrier_bytes: usize,
    /// Maximum opaque carrier bytes retained per session.
    pub max_opaque_carrier_bytes_per_session: usize,
    /// Maximum deferred restore carriers retained before a codec appears.
    pub max_deferred_restore_carriers: usize,
    /// Maximum codec rows accepted per Domain.
    pub max_codecs_per_domain: usize,
    /// Maximum `DomainTable` entries per attach.
    pub max_domain_table_entries: usize,
    /// Maximum server-rendered codec display bytes.
    pub max_codec_display_bytes: usize,
    /// Maximum invalid-carrier DS12 events per source/minute.
    pub max_invalid_carrier_per_min: u32,
    /// Maximum live streams per session.
    pub max_streams_per_session: usize,
    /// Maximum registered stream schemes in one process.
    pub max_stream_schemes: usize,
    /// Maximum stream bytes accepted but not drained.
    pub stream_max_bytes_in_flight: u64,
    /// Maximum kernel-owned stream queued bytes.
    pub stream_max_bytes_buffered: u64,
    /// Stream drain timeout in milliseconds.
    pub stream_drain_timeout_ms: u64,
    /// Maximum subscriptions retained per stream.
    pub max_stream_subscriptions_per_stream: usize,
}

impl KernelStateLimits {
    /// Builds a limits record.
    ///
    /// ```rust
    /// use reovim_kernel::state::KernelStateLimits;
    ///
    /// let limits = KernelStateLimits::new(8, 2, 100);
    /// assert_eq!(limits.max_slots_per_window, 2);
    /// ```
    #[must_use]
    pub const fn new(
        max_view_slot_bytes: usize,
        max_slots_per_window: usize,
        service_lease_timeout_ms: u64,
    ) -> Self {
        let carrier = CarrierKernelLimits::DEFAULT;
        Self {
            max_view_slot_bytes,
            max_slots_per_window,
            service_lease_timeout_ms,
            max_register_carrier_bytes: 4096,
            max_client_registers: 64,
            max_session_registers: 128,
            max_system_registers: 256,
            max_undo_groups_per_buffer: 1024,
            max_edits_per_group: 1024,
            max_undo_bytes_per_buffer: 1024 * 1024,
            max_position_carrier_bytes: carrier.max_position_carrier_bytes,
            max_cursor_carrier_bytes: carrier.max_cursor_carrier_bytes,
            max_opaque_carrier_bytes_per_session: carrier.max_opaque_carrier_bytes_per_session,
            max_deferred_restore_carriers: carrier.max_deferred_restore_carriers,
            max_codecs_per_domain: carrier.max_codecs_per_domain,
            max_domain_table_entries: carrier.max_domain_table_entries,
            max_codec_display_bytes: carrier.max_codec_display_bytes,
            max_invalid_carrier_per_min: carrier.max_invalid_carrier_per_min,
            max_streams_per_session: 128,
            max_stream_schemes: 128,
            stream_max_bytes_in_flight: 8 * 1024 * 1024,
            stream_max_bytes_buffered: 16 * 1024 * 1024,
            stream_drain_timeout_ms: 3000,
            max_stream_subscriptions_per_stream: 256,
        }
    }

    /// Returns a copy with register limits overridden.
    ///
    /// ```rust
    /// use reovim_kernel::state::KernelStateLimits;
    ///
    /// let limits = KernelStateLimits::default().with_register_limits(1, 2, 3, 4);
    /// assert_eq!(limits.max_system_registers, 4);
    /// ```
    #[must_use]
    pub const fn with_register_limits(
        mut self,
        max_register_carrier_bytes: usize,
        max_client_registers: usize,
        max_session_registers: usize,
        max_system_registers: usize,
    ) -> Self {
        self.max_register_carrier_bytes = max_register_carrier_bytes;
        self.max_client_registers = max_client_registers;
        self.max_session_registers = max_session_registers;
        self.max_system_registers = max_system_registers;
        self
    }

    /// Returns a copy with undo limits overridden.
    ///
    /// ```rust
    /// use reovim_kernel::state::KernelStateLimits;
    ///
    /// let limits = KernelStateLimits::default().with_undo_limits(1, 2, 3);
    /// assert_eq!(limits.max_undo_bytes_per_buffer, 3);
    /// ```
    #[must_use]
    pub const fn with_undo_limits(
        mut self,
        max_undo_groups_per_buffer: usize,
        max_edits_per_group: usize,
        max_undo_bytes_per_buffer: usize,
    ) -> Self {
        self.max_undo_groups_per_buffer = max_undo_groups_per_buffer;
        self.max_edits_per_group = max_edits_per_group;
        self.max_undo_bytes_per_buffer = max_undo_bytes_per_buffer;
        self
    }

    /// Returns a copy with carrier/codec limits overridden.
    ///
    /// ```rust
    /// use reovim_kernel::state::{CarrierKernelLimits, KernelStateLimits};
    ///
    /// let carrier = CarrierKernelLimits {
    ///     max_codecs_per_domain: 5,
    ///     ..CarrierKernelLimits::default()
    /// };
    /// let limits = KernelStateLimits::default().with_carrier_limits(carrier);
    /// assert_eq!(limits.max_codecs_per_domain, 5);
    /// ```
    #[must_use]
    pub const fn with_carrier_limits(mut self, limits: CarrierKernelLimits) -> Self {
        self.max_position_carrier_bytes = limits.max_position_carrier_bytes;
        self.max_cursor_carrier_bytes = limits.max_cursor_carrier_bytes;
        self.max_opaque_carrier_bytes_per_session = limits.max_opaque_carrier_bytes_per_session;
        self.max_deferred_restore_carriers = limits.max_deferred_restore_carriers;
        self.max_codecs_per_domain = limits.max_codecs_per_domain;
        self.max_domain_table_entries = limits.max_domain_table_entries;
        self.max_codec_display_bytes = limits.max_codec_display_bytes;
        self.max_invalid_carrier_per_min = limits.max_invalid_carrier_per_min;
        self
    }

    /// Returns a copy with stream limits overridden.
    ///
    /// ```rust
    /// use reovim_kernel::state::KernelStateLimits;
    ///
    /// let limits = KernelStateLimits::default().with_stream_limits(1, 2, 3, 4, 5);
    /// assert_eq!(limits.stream_drain_timeout_ms, 5);
    /// ```
    #[must_use]
    pub const fn with_stream_limits(
        mut self,
        max_streams_per_session: usize,
        max_stream_schemes: usize,
        stream_max_bytes_in_flight: u64,
        stream_max_bytes_buffered: u64,
        stream_drain_timeout_ms: u64,
    ) -> Self {
        self.max_streams_per_session = max_streams_per_session;
        self.max_stream_schemes = max_stream_schemes;
        self.stream_max_bytes_in_flight = stream_max_bytes_in_flight;
        self.stream_max_bytes_buffered = stream_max_bytes_buffered;
        self.stream_drain_timeout_ms = stream_drain_timeout_ms;
        self
    }

    const fn undo_limits(self) -> UndoLimits {
        UndoLimits {
            groups_per_buffer: self.max_undo_groups_per_buffer,
            edits_per_group: self.max_edits_per_group,
            bytes_per_buffer: self.max_undo_bytes_per_buffer,
        }
    }
}

impl Default for KernelStateLimits {
    fn default() -> Self {
        Self::new(4096, 64, 30_000)
    }
}

/// Kernel state substrate backing Phase 3 fixtures.
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// use reovim_kernel::state::StateSubstrate;
///
/// let state = StateSubstrate::new();
/// assert!(state.buffer_count() == 0);
/// ```
pub struct StateSubstrate {
    limits: KernelStateLimits,
    buffers: Map<BufferId, BufferRecord>,
    registers: Map<RegisterAddress, CursorCarrier>,
    view_slots: Map<ViewSlotKey, ViewSlotRecord>,
    codecs: CodecRegistry,
    streams: StreamRegistry,
    /// Kernel-owned service registry rows and leases.
    pub service_registry: ServiceRegistry,
}

impl StateSubstrate {
    /// Creates an empty state substrate with default limits.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// use reovim_kernel::state::StateSubstrate;
    ///
    /// let state = StateSubstrate::new();
    /// assert_eq!(state.buffer_count(), 0);
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self::with_limits(KernelStateLimits::default())
    }

    /// Creates an empty state substrate with explicit limits.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// use reovim_kernel::state::{KernelStateLimits, StateSubstrate};
    ///
    /// let state = StateSubstrate::with_limits(KernelStateLimits::new(32, 1, 10));
    /// assert_eq!(state.limits().max_view_slot_bytes, 32);
    /// ```
    #[must_use]
    pub const fn with_limits(limits: KernelStateLimits) -> Self {
        Self {
            limits,
            buffers: Map::new(),
            registers: Map::new(),
            view_slots: Map::new(),
            codecs: CodecRegistry::new(limits),
            streams: StreamRegistry::new(limits),
            service_registry: ServiceRegistry::new(limits.service_lease_timeout_ms),
        }
    }

    /// Returns the active state-substrate limits.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// use reovim_kernel::state::StateSubstrate;
    ///
    /// assert!(StateSubstrate::new().limits().max_slots_per_window > 0);
    /// ```
    #[must_use]
    pub const fn limits(&self) -> KernelStateLimits {
        self.limits
    }

    /// Returns the number of byte-buffer records.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// use reovim_kernel::state::StateSubstrate;
    ///
    /// assert_eq!(StateSubstrate::new().buffer_count(), 0);
    /// ```
    #[must_use]
    pub const fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    #[cfg(feature = "selftest")]
    pub(crate) const fn codec_registry(&self) -> &CodecRegistry {
        &self.codecs
    }

    #[cfg(feature = "selftest")]
    pub(crate) const fn stream_registry(&self) -> &StreamRegistry {
        &self.streams
    }

    pub(crate) fn register_position_codec(
        &mut self,
        domain_id: DomainId,
        inner_id: u16,
        owner_cdylib_id: CdylibId,
        owner_generation: u64,
        max_content_bytes: usize,
    ) -> Result<(), &'static str> {
        if max_content_bytes > self.limits.max_position_carrier_bytes {
            return Err("codec: content limit exceeds kernel cap");
        }
        self.codecs.register(CodecRowRecord::new(
            CarrierKind::Position,
            domain_id,
            inner_id,
            owner_cdylib_id,
            owner_generation,
            max_content_bytes,
        ))
    }

    pub(crate) fn register_cursor_codec(
        &mut self,
        domain_id: DomainId,
        inner_id: u16,
        owner_cdylib_id: CdylibId,
        owner_generation: u64,
        max_content_bytes: usize,
    ) -> Result<(), &'static str> {
        if max_content_bytes > self.limits.max_cursor_carrier_bytes {
            return Err("codec: content limit exceeds kernel cap");
        }
        self.codecs.register(CodecRowRecord::new(
            CarrierKind::Cursor,
            domain_id,
            inner_id,
            owner_cdylib_id,
            owner_generation,
            max_content_bytes,
        ))
    }

    pub(crate) fn revoke_codec_owner(
        &mut self,
        owner_cdylib_id: CdylibId,
        next_generation: u64,
    ) -> Result<usize, &'static str> {
        self.codecs.revoke_owner(owner_cdylib_id, next_generation)
    }

    pub(crate) fn validate_position_carrier(
        &mut self,
        session_id: SessionId,
        source_id: u32,
        now_ms: u64,
        carrier: &PositionCarrier,
    ) -> Result<CarrierValidationReport, &'static str> {
        self.validate_carrier(CarrierValidationInput::new(
            CarrierKind::Position,
            session_id,
            source_id,
            now_ms,
            carrier.header.domain_raw(),
            carrier.header.inner_id(),
            carrier.content.len(),
        ))
    }

    pub(crate) fn validate_cursor_carrier(
        &mut self,
        session_id: SessionId,
        source_id: u32,
        now_ms: u64,
        carrier: &CursorCarrier,
    ) -> Result<CarrierValidationReport, &'static str> {
        self.validate_carrier(CarrierValidationInput::new(
            CarrierKind::Cursor,
            session_id,
            source_id,
            now_ms,
            carrier.header.domain_raw(),
            carrier.header.inner_id(),
            carrier.content.len(),
        ))
    }

    pub(crate) fn register_stream_scheme(
        &mut self,
        scheme: StreamScheme,
        phase: RegistrationPhase,
    ) -> Result<(), &'static str> {
        self.streams.register_scheme(scheme, phase)
    }

    pub(crate) fn open_stream(
        &mut self,
        scheme_name: &[u8],
        session_id: Option<SessionId>,
        buffer_id: Option<BufferId>,
    ) -> Result<StreamId, &'static str> {
        self.streams.open_stream(scheme_name, session_id, buffer_id)
    }

    pub(crate) fn stream_subscribe(
        &mut self,
        stream_id: StreamId,
        session_id: SessionId,
        buffer_id: BufferId,
        max_buffered_bytes: usize,
    ) -> Result<bool, &'static str> {
        self.streams
            .subscribe(stream_id, session_id, buffer_id, max_buffered_bytes)
    }

    pub(crate) fn stream_emit(
        &mut self,
        stream_id: StreamId,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<BackpressureCounters, &'static str> {
        self.streams.emit(stream_id, bytes, now_ms)
    }

    pub(crate) fn stream_drain_subscription(
        &mut self,
        stream_id: StreamId,
        buffer_id: BufferId,
        now_ms: u64,
    ) -> Result<Option<Bytes>, &'static str> {
        self.streams
            .drain_subscription(stream_id, buffer_id, now_ms)
    }

    pub(crate) fn stream_drain_to_buffer(
        &mut self,
        stream_id: StreamId,
        buffer_id: BufferId,
        now_ms: u64,
    ) -> Result<bool, &'static str> {
        let Some(bytes) = self
            .streams
            .drain_subscription(stream_id, buffer_id, now_ms)?
        else {
            return Ok(false);
        };
        if bytes.is_empty() {
            return Ok(false);
        }
        let end = self
            .buffer(buffer_id)
            .map_or(0, |buffer| buffer.bytes().len());
        self.apply_buffer_edit(
            buffer_id,
            EditOrigin::External,
            end,
            end,
            bytes.as_slice(),
            now_ms,
        )?;
        Ok(true)
    }

    pub(crate) fn stream_mark_stale(&mut self, stream_id: StreamId) -> Result<(), &'static str> {
        self.streams.mark_stale(stream_id)
    }

    pub(crate) fn stream_control_class(
        &self,
        stream_id: StreamId,
        op: StreamControlOp,
    ) -> Result<StreamControlClass, &'static str> {
        self.streams.control_class(stream_id, op)
    }

    pub(crate) fn stream_close(&mut self, stream_id: StreamId) -> Result<(), &'static str> {
        self.streams.close(stream_id)
    }

    pub(crate) fn revoke_stream_owner(
        &mut self,
        owner_cdylib_id: CdylibId,
    ) -> Result<usize, &'static str> {
        self.streams.revoke_owner(owner_cdylib_id)
    }

    /// Returns the buffer record for `id`, if present.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub fn buffer(&self, id: BufferId) -> Option<&BufferRecord> {
        self.buffers.get(&id)
    }

    fn validate_carrier(
        &mut self,
        input: CarrierValidationInput,
    ) -> Result<CarrierValidationReport, &'static str> {
        let status = self.codecs.validate(
            input.kind,
            input.session_id,
            input.domain_raw,
            input.inner_id,
            input.content_len,
        );
        let emit_ds12 = if status == CarrierStatus::Invalid {
            self.codecs
                .should_emit_validation_error(input.source_id, input.now_ms)?
        } else {
            false
        };
        Ok(CarrierValidationReport::new(status, emit_ds12))
    }

    /// Ensures a buffer record exists and returns it mutably.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the buffer map cannot grow.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn ensure_buffer(&mut self, id: BufferId) -> Result<&mut BufferRecord, &'static str> {
        if self.buffers.get(&id).is_none() {
            self.buffers
                .try_insert(id, BufferRecord::new_with_limits(id, self.limits.undo_limits()))
                .map_err(|_| "alloc")?;
        }
        self.buffers.get_mut(&id).ok_or("state: missing buffer")
    }

    /// Applies a byte edit to a buffer record, creating it if needed.
    ///
    /// # Errors
    ///
    /// Returns range, cap, or allocation errors from the buffer record.
    pub fn apply_buffer_edit(
        &mut self,
        buffer_id: BufferId,
        origin: EditOrigin,
        start: usize,
        end: usize,
        new_bytes: &[u8],
        timestamp_ms: u64,
    ) -> Result<(), &'static str> {
        self.ensure_buffer(buffer_id)?
            .apply_edit(origin, start, end, new_bytes, timestamp_ms)
    }

    /// Opens an undo group on a buffer record, creating it if needed.
    ///
    /// # Errors
    ///
    /// Returns an error if a group is already open, the origin is not recorded,
    /// or allocation fails.
    pub fn open_buffer_undo_group(
        &mut self,
        buffer_id: BufferId,
        origin: EditOrigin,
        timestamp_ms: u64,
    ) -> Result<UndoGroupId, &'static str> {
        self.ensure_buffer(buffer_id)?
            .open_undo_group(origin, timestamp_ms)
    }

    /// Closes the current undo group on `buffer_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if no group is open or buffer creation fails.
    pub fn close_buffer_undo_group(&mut self, buffer_id: BufferId) -> Result<(), &'static str> {
        self.ensure_buffer(buffer_id)?.close_undo_group()
    }

    /// Undoes the newest group on `buffer_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if a group is open or allocation fails.
    pub fn undo_buffer(&mut self, buffer_id: BufferId) -> Result<bool, &'static str> {
        self.ensure_buffer(buffer_id)?.undo()
    }

    /// Redoes the newest group on `buffer_id`.
    ///
    /// # Errors
    ///
    /// Returns an error if replaying redo bytes or allocation fails.
    pub fn redo_buffer(&mut self, buffer_id: BufferId) -> Result<bool, &'static str> {
        self.ensure_buffer(buffer_id)?.redo()
    }

    /// Returns cloned bytes for `buffer_id`.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if cloning the buffer bytes fails.
    pub fn buffer_bytes(&self, buffer_id: BufferId) -> Result<Option<Bytes>, &'static str> {
        self.buffers
            .get(&buffer_id)
            .map(|buffer| clone_bytes(buffer.bytes()))
            .transpose()
    }

    /// Stores a cursor carrier in the requested register scope.
    ///
    /// # Errors
    ///
    /// Returns an error when carrier structural validation fails or storage
    /// allocation fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn set_register(
        &mut self,
        scope: RegisterScope,
        client_id: ClientId,
        session_id: SessionId,
        id: RegisterId,
        carrier: &CursorCarrier,
    ) -> Result<(), &'static str> {
        if carrier.structural_status() == CarrierStatus::Invalid {
            return Err("register: invalid carrier");
        }
        if carrier.content.len() > self.limits.max_register_carrier_bytes {
            return Err("register: carrier bytes limit");
        }
        let key = RegisterAddress::new(scope, client_id, session_id, id);
        if self.register_limit_exceeded(key) {
            return Err("register: scope limit");
        }
        let cloned = clone_cursor_carrier(carrier)?;
        self.registers
            .try_insert(key, cloned)
            .map_err(|_| "alloc")?;
        Ok(())
    }

    /// Looks up a register with `client -> session -> system` fallback.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub fn lookup_register(
        &self,
        client_id: ClientId,
        session_id: SessionId,
        id: RegisterId,
    ) -> Option<&CursorCarrier> {
        self.registers
            .get(&RegisterAddress::new(RegisterScope::Client, client_id, session_id, id))
            .or_else(|| {
                self.registers.get(&RegisterAddress::new(
                    RegisterScope::Session,
                    client_id,
                    session_id,
                    id,
                ))
            })
            .or_else(|| {
                self.registers.get(&RegisterAddress::new(
                    RegisterScope::System,
                    client_id,
                    session_id,
                    id,
                ))
            })
    }

    /// Looks up and clones a register carrier.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if cloning the carrier bytes fails.
    pub fn lookup_register_cloned(
        &self,
        client_id: ClientId,
        session_id: SessionId,
        id: RegisterId,
    ) -> Result<Option<CursorCarrier>, &'static str> {
        self.lookup_register(client_id, session_id, id)
            .map(clone_cursor_carrier)
            .transpose()
    }

    /// Clears a register value in exactly one scope.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn clear_register(
        &mut self,
        scope: RegisterScope,
        client_id: ClientId,
        session_id: SessionId,
        id: RegisterId,
    ) -> bool {
        self.registers
            .remove(&RegisterAddress::new(scope, client_id, session_id, id))
            .is_some()
    }

    /// Stores or replaces a view slot.
    ///
    /// # Errors
    ///
    /// Returns an error when a limit is exceeded or byte storage allocation
    /// fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn put_view_slot(
        &mut self,
        key: ViewSlotKey,
        owner_cdylib_id: CdylibId,
        flags: ViewSlotFlags,
        bytes: &[u8],
    ) -> Result<(), &'static str> {
        if bytes.len() > self.limits.max_view_slot_bytes {
            return Err("view-slot: bytes limit");
        }
        if self.window_slot_limit_exceeded(key) {
            return Err("view-slot: window slot limit");
        }
        let record = ViewSlotRecord::new(key, owner_cdylib_id, flags, clone_bytes_slice(bytes)?);
        self.view_slots
            .try_insert(key, record)
            .map_err(|_| "alloc")?;
        Ok(())
    }

    /// Returns a view slot by exact key.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub fn view_slot(&self, key: ViewSlotKey) -> Option<&ViewSlotRecord> {
        self.view_slots.get(&key)
    }

    /// Returns a cloned view-slot record by exact key.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if cloning the slot bytes fails.
    pub fn view_slot_cloned(
        &self,
        key: ViewSlotKey,
    ) -> Result<Option<ViewSlotRecord>, &'static str> {
        self.view_slots
            .get(&key)
            .map(ViewSlotRecord::try_clone)
            .transpose()
    }

    /// Drops all view slots owned by `owner_cdylib_id`.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the temporary key list cannot grow.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn drop_owner_view_slots(
        &mut self,
        owner_cdylib_id: CdylibId,
    ) -> Result<usize, &'static str> {
        let mut keys = Seq::new();
        for (key, slot) in &self.view_slots {
            if slot.owner_cdylib_id == owner_cdylib_id {
                keys.try_push(*key).map_err(|_| "alloc")?;
            }
        }
        for key in keys.as_slice() {
            self.view_slots.remove(key);
        }
        Ok(keys.len())
    }

    /// Drops all window-scoped slots for one `(client, buffer, window)`.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the temporary key list cannot grow.
    pub fn drop_window_view_slots(
        &mut self,
        client_id: ClientId,
        buffer_id: BufferId,
        window_id: WindowId,
    ) -> Result<usize, &'static str> {
        let mut keys = Seq::new();
        for (key, _) in &self.view_slots {
            if let ViewSlotKey::Window {
                client_id: existing_client,
                buffer_id: existing_buffer,
                window_id: existing_window,
                ..
            } = *key
                && existing_client == client_id
                && existing_buffer == buffer_id
                && existing_window == window_id
            {
                keys.try_push(*key).map_err(|_| "alloc")?;
            }
        }
        for key in keys.as_slice() {
            self.view_slots.remove(key);
        }
        Ok(keys.len())
    }

    /// Drops all window and buffer-scoped slots associated with `buffer_id`.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the temporary key list cannot grow.
    pub fn drop_buffer_view_slots(&mut self, buffer_id: BufferId) -> Result<usize, &'static str> {
        let mut keys = Seq::new();
        for (key, _) in &self.view_slots {
            if key.buffer_id() == buffer_id {
                keys.try_push(*key).map_err(|_| "alloc")?;
            }
        }
        for key in keys.as_slice() {
            self.view_slots.remove(key);
        }
        Ok(keys.len())
    }

    fn window_slot_limit_exceeded(&self, key: ViewSlotKey) -> bool {
        let ViewSlotKey::Window {
            client_id,
            buffer_id,
            window_id,
            ..
        } = key
        else {
            return false;
        };
        if self.view_slots.get(&key).is_some() {
            return false;
        }
        let mut count = 0usize;
        for (existing, _) in &self.view_slots {
            if let ViewSlotKey::Window {
                client_id: existing_client,
                buffer_id: existing_buffer,
                window_id: existing_window,
                ..
            } = *existing
                && existing_client == client_id
                && existing_buffer == buffer_id
                && existing_window == window_id
            {
                count += 1;
            }
        }
        count >= self.limits.max_slots_per_window
    }

    fn register_limit_exceeded(&self, key: RegisterAddress) -> bool {
        if self.registers.get(&key).is_some() {
            return false;
        }
        let mut count = 0usize;
        for (existing, _) in &self.registers {
            if existing.is_same_limit_bucket(key) {
                count += 1;
            }
        }
        count
            >= match key.key.scope {
                RegisterScope::Client => self.limits.max_client_registers,
                RegisterScope::Session => self.limits.max_session_registers,
                RegisterScope::System => self.limits.max_system_registers,
            }
    }
}

impl Default for StateSubstrate {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum CarrierKind {
    Position,
    Cursor,
}

impl CarrierKind {
    const fn max_content_bytes(self, limits: KernelStateLimits) -> usize {
        match self {
            Self::Position => limits.max_position_carrier_bytes,
            Self::Cursor => limits.max_cursor_carrier_bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CarrierValidationReport {
    pub status: CarrierStatus,
    pub emit_ds12: bool,
}

impl CarrierValidationReport {
    const fn new(status: CarrierStatus, emit_ds12: bool) -> Self {
        Self { status, emit_ds12 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CarrierValidationInput {
    kind: CarrierKind,
    session_id: SessionId,
    source_id: u32,
    now_ms: u64,
    domain_raw: u32,
    inner_id: u16,
    content_len: usize,
}

impl CarrierValidationInput {
    const fn new(
        kind: CarrierKind,
        session_id: SessionId,
        source_id: u32,
        now_ms: u64,
        domain_raw: u32,
        inner_id: u16,
        content_len: usize,
    ) -> Self {
        Self {
            kind,
            session_id,
            source_id,
            now_ms,
            domain_raw,
            inner_id,
            content_len,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CodecKey {
    kind: CarrierKind,
    domain_id: DomainId,
    inner_id: u16,
}

impl CodecKey {
    const fn new(kind: CarrierKind, domain_id: DomainId, inner_id: u16) -> Self {
        Self {
            kind,
            domain_id,
            inner_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodecRowState {
    Active,
    Revoked,
}

impl CodecRowState {
    const fn is_active(self) -> bool {
        matches!(self, Self::Active)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CodecRowRecord {
    key: CodecKey,
    owner_cdylib_id: CdylibId,
    owner_generation: u64,
    max_content_bytes: usize,
    state: CodecRowState,
}

impl CodecRowRecord {
    const fn new(
        kind: CarrierKind,
        domain_id: DomainId,
        inner_id: u16,
        owner_cdylib_id: CdylibId,
        owner_generation: u64,
        max_content_bytes: usize,
    ) -> Self {
        Self {
            key: CodecKey::new(kind, domain_id, inner_id),
            owner_cdylib_id,
            owner_generation,
            max_content_bytes,
            state: CodecRowState::Active,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct ValidationSource(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValidationRateBucket {
    minute_start_ms: u64,
    emitted: u32,
}

impl ValidationRateBucket {
    const fn new(minute_start_ms: u64) -> Self {
        Self {
            minute_start_ms,
            emitted: 1,
        }
    }
}

pub(crate) struct CodecRegistry {
    limits: KernelStateLimits,
    rows: Map<CodecKey, CodecRowRecord>,
    opaque_bytes_by_session: Map<SessionId, usize>,
    validation_errors: Map<ValidationSource, ValidationRateBucket>,
}

impl CodecRegistry {
    const fn new(limits: KernelStateLimits) -> Self {
        Self {
            limits,
            rows: Map::new(),
            opaque_bytes_by_session: Map::new(),
            validation_errors: Map::new(),
        }
    }

    fn row(
        &self,
        kind: CarrierKind,
        domain_id: DomainId,
        inner_id: u16,
    ) -> Option<&CodecRowRecord> {
        self.rows.get(&CodecKey::new(kind, domain_id, inner_id))
    }

    #[cfg(feature = "selftest")]
    pub(crate) fn len(&self) -> usize {
        self.rows.len()
    }

    fn register(&mut self, row: CodecRowRecord) -> Result<(), &'static str> {
        if row.max_content_bytes > row.key.kind.max_content_bytes(self.limits) {
            return Err("codec: content limit exceeds kernel cap");
        }
        if let Some(existing) = self.rows.get(&row.key) {
            if existing.owner_cdylib_id != row.owner_cdylib_id {
                return Err("codec: duplicate key");
            }
            if row.owner_generation < existing.owner_generation {
                return Err("codec: stale owner generation");
            }
            self.rows.try_insert(row.key, row).map_err(|_| "alloc")?;
            return Ok(());
        }
        if self.domain_codec_count(row.key.domain_id) >= self.limits.max_codecs_per_domain {
            return Err("codec: domain codec cap");
        }
        self.rows.try_insert(row.key, row).map_err(|_| "alloc")?;
        Ok(())
    }

    fn validate(
        &mut self,
        kind: CarrierKind,
        session_id: SessionId,
        domain_raw: u32,
        inner_id: u16,
        content_len: usize,
    ) -> CarrierStatus {
        if content_len > kind.max_content_bytes(self.limits) {
            return CarrierStatus::Invalid;
        }
        if domain_raw == 0 {
            return if content_len == 0 {
                CarrierStatus::ValidOpaque
            } else {
                CarrierStatus::Invalid
            };
        }
        let Some(raw) = core::num::NonZeroU32::new(domain_raw) else {
            return CarrierStatus::Invalid;
        };
        let domain_id = DomainId::new(raw);
        if let Some(row) = self.row(kind, domain_id, inner_id) {
            if !row.state.is_active() {
                return self.record_opaque_bytes(session_id, content_len);
            }
            if content_len > row.max_content_bytes {
                return CarrierStatus::Invalid;
            }
            return CarrierStatus::ValidKnown;
        }
        self.record_opaque_bytes(session_id, content_len)
    }

    fn revoke_owner(
        &mut self,
        owner_cdylib_id: CdylibId,
        next_generation: u64,
    ) -> Result<usize, &'static str> {
        let mut keys = Seq::new();
        for (key, row) in &self.rows {
            if row.owner_cdylib_id == owner_cdylib_id {
                keys.try_push(*key).map_err(|_| "alloc")?;
            }
        }
        let mut revoked = 0usize;
        for key in keys.as_slice() {
            if let Some(row) = self.rows.get_mut(key) {
                row.owner_generation = next_generation;
                row.state = CodecRowState::Revoked;
                revoked += 1;
            }
        }
        Ok(revoked)
    }

    fn should_emit_validation_error(
        &mut self,
        source_id: u32,
        now_ms: u64,
    ) -> Result<bool, &'static str> {
        if self.limits.max_invalid_carrier_per_min == 0 {
            return Ok(false);
        }
        let key = ValidationSource(source_id);
        let minute_start_ms = now_ms - (now_ms % 60_000);
        if let Some(bucket) = self.validation_errors.get_mut(&key) {
            if bucket.minute_start_ms != minute_start_ms {
                *bucket = ValidationRateBucket::new(minute_start_ms);
                return Ok(true);
            }
            if bucket.emitted >= self.limits.max_invalid_carrier_per_min {
                return Ok(false);
            }
            bucket.emitted += 1;
            return Ok(true);
        }
        self.validation_errors
            .try_insert(key, ValidationRateBucket::new(minute_start_ms))
            .map_err(|_| "alloc")?;
        Ok(true)
    }

    fn domain_codec_count(&self, domain_id: DomainId) -> usize {
        let mut count = 0usize;
        for (key, _) in &self.rows {
            if key.domain_id == domain_id {
                count += 1;
            }
        }
        count
    }

    fn record_opaque_bytes(&mut self, session_id: SessionId, bytes: usize) -> CarrierStatus {
        if bytes == 0 {
            return CarrierStatus::ValidOpaque;
        }
        let current = self
            .opaque_bytes_by_session
            .get(&session_id)
            .copied()
            .unwrap_or(0);
        let Some(next) = current.checked_add(bytes) else {
            return CarrierStatus::Invalid;
        };
        if next > self.limits.max_opaque_carrier_bytes_per_session {
            return CarrierStatus::Invalid;
        }
        if self
            .opaque_bytes_by_session
            .try_insert(session_id, next)
            .is_err()
        {
            return CarrierStatus::Invalid;
        }
        CarrierStatus::ValidOpaque
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SchemeNameKey([u8; 32]);

impl SchemeNameKey {
    fn from_bytes(name: &[u8]) -> Result<Self, &'static str> {
        if name.is_empty() || name.len() > 31 {
            return Err("stream: invalid scheme name");
        }
        let mut key = [0u8; 32];
        key[..name.len()].copy_from_slice(name);
        Ok(Self(key))
    }
}

struct StreamSubscriptionRecord {
    session_id: SessionId,
    buffer_id: BufferId,
    max_buffered_bytes: usize,
    queue: Bytes,
}

impl StreamSubscriptionRecord {
    const fn new(session_id: SessionId, buffer_id: BufferId, max_buffered_bytes: usize) -> Self {
        Self {
            session_id,
            buffer_id,
            max_buffered_bytes,
            queue: Bytes::new(),
        }
    }
}

struct StreamHandleRecord {
    handle: StreamHandle,
    owner_cdylib_id: CdylibId,
    subscriptions: Seq<StreamSubscriptionRecord>,
}

impl StreamHandleRecord {
    const fn new(handle: StreamHandle, owner_cdylib_id: CdylibId) -> Self {
        Self {
            handle,
            owner_cdylib_id,
            subscriptions: Seq::new(),
        }
    }
}

pub(crate) struct StreamRegistry {
    limits: KernelStateLimits,
    schemes: Map<SchemeNameKey, StreamScheme>,
    handles: Map<StreamId, StreamHandleRecord>,
    next_stream_id: u64,
}

impl StreamRegistry {
    const fn new(limits: KernelStateLimits) -> Self {
        Self {
            limits,
            schemes: Map::new(),
            handles: Map::new(),
            next_stream_id: 1,
        }
    }

    #[cfg(feature = "selftest")]
    pub(crate) fn scheme_count(&self) -> usize {
        self.schemes.len()
    }

    #[cfg(feature = "selftest")]
    pub(crate) fn handle(&self, stream_id: StreamId) -> Option<&StreamHandle> {
        self.handles.get(&stream_id).map(|record| &record.handle)
    }

    #[cfg(feature = "selftest")]
    pub(crate) fn subscription_count(&self, stream_id: StreamId) -> usize {
        self.handles
            .get(&stream_id)
            .map(|record| record.subscriptions.len())
            .unwrap_or(0)
    }

    fn register_scheme(
        &mut self,
        scheme: StreamScheme,
        phase: RegistrationPhase,
    ) -> Result<(), &'static str> {
        if phase != RegistrationPhase::Init {
            return Err("stream: registration outside init");
        }
        if self.schemes.len() >= self.limits.max_stream_schemes {
            return Err("stream: scheme cap");
        }
        let key = SchemeNameKey::from_bytes(scheme.name.as_slice())?;
        if self.schemes.get(&key).is_some() {
            return Err("stream: duplicate scheme");
        }
        self.schemes.try_insert(key, scheme).map_err(|_| "alloc")?;
        Ok(())
    }

    fn open_stream(
        &mut self,
        scheme_name: &[u8],
        session_id: Option<SessionId>,
        buffer_id: Option<BufferId>,
    ) -> Result<StreamId, &'static str> {
        let key = SchemeNameKey::from_bytes(scheme_name)?;
        let scheme = self.schemes.get(&key).ok_or("stream: missing scheme")?;
        if let Some(session) = session_id
            && self.session_stream_count(session) >= self.limits.max_streams_per_session
        {
            return Err("stream: session cap");
        }
        let id = StreamId::new(self.next_stream_id);
        self.next_stream_id = self
            .next_stream_id
            .checked_add(1)
            .ok_or("stream: id overflow")?;
        let handle = StreamHandle::new(
            id,
            clone_bytes(&scheme.name)?,
            StreamState::Running,
            session_id,
            buffer_id,
            BackpressureCounters::new(0, 0, 0),
        );
        let record = StreamHandleRecord::new(handle, scheme.owner_cdylib_id);
        self.handles.try_insert(id, record).map_err(|_| "alloc")?;
        Ok(id)
    }

    fn subscribe(
        &mut self,
        stream_id: StreamId,
        session_id: SessionId,
        buffer_id: BufferId,
        max_buffered_bytes: usize,
    ) -> Result<bool, &'static str> {
        let record = self
            .handles
            .get_mut(&stream_id)
            .ok_or("stream: missing handle")?;
        if record.subscriptions.len() >= self.limits.max_stream_subscriptions_per_stream {
            return Err("stream: subscription cap");
        }
        for sub in record.subscriptions.as_slice() {
            if sub.session_id == session_id && sub.buffer_id == buffer_id {
                return Ok(false);
            }
        }
        record
            .subscriptions
            .try_push(StreamSubscriptionRecord::new(session_id, buffer_id, max_buffered_bytes))
            .map_err(|_| "alloc")?;
        Ok(true)
    }

    fn emit(
        &mut self,
        stream_id: StreamId,
        bytes: &[u8],
        now_ms: u64,
    ) -> Result<BackpressureCounters, &'static str> {
        let record = self
            .handles
            .get_mut(&stream_id)
            .ok_or("stream: missing handle")?;
        if !record.handle.state.accepts_emit() {
            return Err("stream: not accepting emit");
        }
        let added_bytes = u64::try_from(bytes.len()).map_err(|_| "stream: byte cap")?;
        let subscription_count =
            u64::try_from(record.subscriptions.len()).map_err(|_| "stream: subscription cap")?;
        let added_buffered = added_bytes
            .checked_mul(subscription_count)
            .ok_or("stream: byte cap")?;
        let next_in_flight = record
            .handle
            .backpressure
            .bytes_in_flight
            .checked_add(added_buffered)
            .ok_or("stream: byte cap")?;
        if next_in_flight > self.limits.stream_max_bytes_in_flight {
            record.handle.state = StreamState::BackpressureBlocked;
            return Err("stream: bytes-in-flight backpressure");
        }
        let next_buffered = record
            .handle
            .backpressure
            .bytes_buffered
            .checked_add(added_buffered)
            .ok_or("stream: byte cap")?;
        if next_buffered > self.limits.stream_max_bytes_buffered {
            record.handle.state = StreamState::BackpressureBlocked;
            return Err("stream: buffered backpressure");
        }
        for sub in record.subscriptions.as_mut_slice() {
            let next_len = sub
                .queue
                .len()
                .checked_add(bytes.len())
                .ok_or("stream: byte cap")?;
            if next_len > sub.max_buffered_bytes {
                record.handle.state = StreamState::BackpressureBlocked;
                return Err("stream: subscription backpressure");
            }
        }
        for sub in record.subscriptions.as_mut_slice() {
            sub.queue
                .try_extend_from_slice(bytes)
                .map_err(|_| "alloc")?;
        }
        record.handle.backpressure =
            BackpressureCounters::new(next_in_flight, next_buffered, now_ms);
        record.handle.state = StreamState::Running;
        Ok(record.handle.backpressure)
    }

    fn drain_subscription(
        &mut self,
        stream_id: StreamId,
        buffer_id: BufferId,
        now_ms: u64,
    ) -> Result<Option<Bytes>, &'static str> {
        let record = self
            .handles
            .get_mut(&stream_id)
            .ok_or("stream: missing handle")?;
        for sub in record.subscriptions.as_mut_slice() {
            if sub.buffer_id == buffer_id {
                let drained = core::mem::take(&mut sub.queue);
                let drained_len = u64::try_from(drained.len()).map_err(|_| "stream: byte cap")?;
                record.handle.backpressure.bytes_in_flight = record
                    .handle
                    .backpressure
                    .bytes_in_flight
                    .saturating_sub(drained_len);
                record.handle.backpressure.bytes_buffered = record
                    .handle
                    .backpressure
                    .bytes_buffered
                    .saturating_sub(drained_len);
                record.handle.backpressure.last_drain_at_ms = now_ms;
                if record.handle.state == StreamState::BackpressureBlocked
                    && record
                        .handle
                        .backpressure
                        .blocked_by(
                            self.limits.stream_max_bytes_in_flight,
                            self.limits.stream_max_bytes_buffered,
                        )
                        .is_none()
                {
                    record.handle.state = StreamState::Running;
                }
                return Ok(Some(drained));
            }
        }
        Ok(None)
    }

    fn mark_stale(&mut self, stream_id: StreamId) -> Result<(), &'static str> {
        let record = self
            .handles
            .get_mut(&stream_id)
            .ok_or("stream: missing handle")?;
        record.handle.state = StreamState::Stale;
        Ok(())
    }

    fn control_class(
        &self,
        stream_id: StreamId,
        op: StreamControlOp,
    ) -> Result<StreamControlClass, &'static str> {
        self.handles
            .get(&stream_id)
            .ok_or("stream: missing handle")?;
        match op.class() {
            StreamControlClass::Invalid => Err("stream: invalid control op"),
            StreamControlClass::Reserved => Err("stream: reserved control op"),
            class => Ok(class),
        }
    }

    fn close(&mut self, stream_id: StreamId) -> Result<(), &'static str> {
        let record = self
            .handles
            .get_mut(&stream_id)
            .ok_or("stream: missing handle")?;
        record.handle.state = StreamState::Closed;
        clear_seq(&mut record.subscriptions);
        record.handle.backpressure =
            BackpressureCounters::new(0, 0, record.handle.backpressure.last_drain_at_ms);
        Ok(())
    }

    fn revoke_owner(&mut self, owner_cdylib_id: CdylibId) -> Result<usize, &'static str> {
        let mut count = 0usize;
        for (_, scheme) in &self.schemes {
            if scheme.owner_cdylib_id == owner_cdylib_id {
                count += 1;
            }
        }
        let mut handles = Seq::new();
        for (stream_id, record) in &self.handles {
            if record.owner_cdylib_id == owner_cdylib_id && !record.handle.state.is_closed() {
                handles.try_push(*stream_id).map_err(|_| "alloc")?;
            }
        }
        for stream_id in handles.as_slice() {
            if let Some(record) = self.handles.get_mut(stream_id) {
                record.handle.state = StreamState::Draining;
                count += 1;
            }
        }
        Ok(count)
    }

    fn session_stream_count(&self, session_id: SessionId) -> usize {
        let mut count = 0usize;
        for (_, record) in &self.handles {
            if record.handle.session_id == Some(session_id) && !record.handle.state.is_closed() {
                count += 1;
            }
        }
        count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UndoLimits {
    groups_per_buffer: usize,
    edits_per_group: usize,
    bytes_per_buffer: usize,
}

impl Default for UndoLimits {
    fn default() -> Self {
        KernelStateLimits::default().undo_limits()
    }
}

/// Byte-buffer state plus byte-range undo/redo stack.
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// use reovim_kernel::state::BufferRecord;
/// use reovim_subsys_domain::id::BufferId;
///
/// let buffer = BufferRecord::new(BufferId::new(1));
/// assert!(buffer.bytes().is_empty());
/// ```
pub struct BufferRecord {
    id: BufferId,
    bytes: Bytes,
    undo: UndoStack,
    next_group_id: u64,
    limits: UndoLimits,
}

impl BufferRecord {
    /// Creates an empty buffer record.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn new(id: BufferId) -> Self {
        Self::new_with_limits(
            id,
            UndoLimits {
                groups_per_buffer: 1024,
                edits_per_group: 1024,
                bytes_per_buffer: 1024 * 1024,
            },
        )
    }

    const fn new_with_limits(id: BufferId, limits: UndoLimits) -> Self {
        Self {
            id,
            bytes: Bytes::new(),
            undo: UndoStack::new(),
            next_group_id: 1,
            limits,
        }
    }

    /// Returns this buffer's id.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn id(&self) -> BufferId {
        self.id
    }

    /// Returns current buffer bytes.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn bytes(&self) -> &Bytes {
        &self.bytes
    }

    /// Returns the undo stack.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn undo_stack(&self) -> &UndoStack {
        &self.undo
    }

    /// Opens an explicit undo group for subsequent edits.
    ///
    /// # Errors
    ///
    /// Returns an error if a group is already open, the origin is not recorded
    /// by the undo stack, the id counter overflows, or allocation fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn open_undo_group(
        &mut self,
        origin: EditOrigin,
        timestamp_ms: u64,
    ) -> Result<UndoGroupId, &'static str> {
        if self.undo.group_open.is_some() {
            return Err("undo: group already open");
        }
        if !records_undo_group(origin) {
            return Err("undo: origin not recorded");
        }
        let id = self.alloc_group_id()?;
        let group = UndoGroup::new(id, origin, timestamp_ms);
        self.undo.groups.try_push(group).map_err(|_| "alloc")?;
        self.undo.group_open = Some(id);
        Ok(id)
    }

    /// Closes the current explicit undo group.
    ///
    /// # Errors
    ///
    /// Returns an error if no group is open.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub const fn close_undo_group(&mut self) -> Result<(), &'static str> {
        if self.undo.group_open.take().is_some() {
            Ok(())
        } else {
            Err("undo: no group open")
        }
    }

    /// Applies a byte-range edit and records undo state when required.
    ///
    /// # Errors
    ///
    /// Returns an error if the range is invalid, allocation fails, or an open
    /// undo group cannot be found.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn apply_edit(
        &mut self,
        origin: EditOrigin,
        start: usize,
        end: usize,
        new_bytes: &[u8],
        timestamp_ms: u64,
    ) -> Result<(), &'static str> {
        validate_range(&self.bytes, start, end)?;
        let replacement = replace_range_bytes(&self.bytes, start, end, new_bytes)?;
        let old_record_bytes = clone_bytes_slice(&self.bytes.as_slice()[start..end])?;
        let new_record_bytes = clone_bytes_slice(new_bytes)?;
        let edit = EditRecord::new(start, end, old_record_bytes, new_record_bytes);

        if let Some(open_id) = self.undo.group_open {
            let group =
                find_group_mut(&mut self.undo.groups, open_id).ok_or("undo: open group missing")?;
            if group.edits.len() >= self.limits.edits_per_group {
                self.undo.group_open = None;
                return Err("undo: edits per group limit");
            }
            group.edits.try_push(edit).map_err(|_| "alloc")?;
        } else if records_undo_group(origin) {
            let id = self.alloc_group_id()?;
            let mut group = UndoGroup::new(id, origin, timestamp_ms);
            group.edits.try_push(edit).map_err(|_| "alloc")?;
            self.undo.groups.try_push(group).map_err(|_| "alloc")?;
        }
        clear_seq(&mut self.undo.redo_groups);
        self.bytes = replacement;
        self.enforce_undo_limits();
        Ok(())
    }

    /// Undoes the newest undoable group, stopping at external markers.
    ///
    /// # Errors
    ///
    /// Returns an error if a group is open or allocation fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn undo(&mut self) -> Result<bool, &'static str> {
        if self.undo.group_open.is_some() {
            return Err("undo: group open");
        }
        self.undo.redo_groups.try_reserve(1).map_err(|_| "alloc")?;
        while let Some(group) = self.undo.groups.pop() {
            if matches!(group.origin, EditOrigin::External) {
                return Ok(false);
            }
            if !group.is_undoable() {
                continue;
            }
            let replacement = apply_group_undo(&self.bytes, &group)?;
            self.undo.redo_groups.try_push(group).map_err(|_| "alloc")?;
            self.bytes = replacement;
            return Ok(true);
        }
        Ok(false)
    }

    /// Redoes the newest redo group.
    ///
    /// # Errors
    ///
    /// Returns an error if allocation fails.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn redo(&mut self) -> Result<bool, &'static str> {
        self.undo.groups.try_reserve(1).map_err(|_| "alloc")?;
        let Some(group) = self.undo.redo_groups.pop() else {
            return Ok(false);
        };
        let replacement = apply_group_redo(&self.bytes, &group)?;
        self.undo.groups.try_push(group).map_err(|_| "alloc")?;
        self.bytes = replacement;
        Ok(true)
    }

    fn alloc_group_id(&mut self) -> Result<UndoGroupId, &'static str> {
        let raw = self.next_group_id;
        self.next_group_id = self
            .next_group_id
            .checked_add(1)
            .ok_or("undo: group id overflow")?;
        Ok(UndoGroupId::new(raw))
    }

    fn enforce_undo_limits(&mut self) {
        while self.undo.groups.len() > self.limits.groups_per_buffer {
            remove_first_group(&mut self.undo.groups);
        }
        while undo_recorded_bytes(&self.undo) > self.limits.bytes_per_buffer {
            if !remove_first_group(&mut self.undo.groups)
                && !remove_first_group(&mut self.undo.redo_groups)
            {
                break;
            }
        }
    }
}

/// Stored opaque view-slot bytes and metadata.
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// ```
pub struct ViewSlotRecord {
    /// Exact view-slot key.
    pub key: ViewSlotKey,
    /// Owner whose unload drops this slot.
    pub owner_cdylib_id: CdylibId,
    /// Slot behavior flags.
    pub flags: ViewSlotFlags,
    /// Opaque owner bytes.
    pub bytes: Bytes,
}

impl ViewSlotRecord {
    /// Builds a view-slot record.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn new(
        key: ViewSlotKey,
        owner_cdylib_id: CdylibId,
        flags: ViewSlotFlags,
        bytes: Bytes,
    ) -> Self {
        Self {
            key,
            owner_cdylib_id,
            flags,
            bytes,
        }
    }

    fn try_clone(&self) -> Result<Self, &'static str> {
        Ok(Self::new(self.key, self.owner_cdylib_id, self.flags, clone_bytes(&self.bytes)?))
    }
}

/// Kernel service registry storage and lease state.
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// use reovim_kernel::state::ServiceRegistry;
///
/// let registry = ServiceRegistry::new(100);
/// assert_eq!(registry.lease_timeout_ms(), 100);
/// ```
pub struct ServiceRegistry {
    rows: Map<ServiceKey, ServiceRowRecord>,
    leases: Map<ServiceLeaseId, ServiceLease>,
    next_lease_id: u64,
    lease_timeout_ms: u64,
}

impl ServiceRegistry {
    /// Creates an empty service registry.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn new(lease_timeout_ms: u64) -> Self {
        Self {
            rows: Map::new(),
            leases: Map::new(),
            next_lease_id: 1,
            lease_timeout_ms,
        }
    }

    /// Returns the configured lease timeout.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn lease_timeout_ms(&self) -> u64 {
        self.lease_timeout_ms
    }

    /// Registers one visible service row.
    ///
    /// # Errors
    ///
    /// Returns an error on duplicate service key or storage allocation failure.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn register(
        &mut self,
        meta: ServiceDescriptorMeta,
        owner_generation: u64,
    ) -> Result<(), &'static str> {
        self.register_with_thread(meta, owner_generation, reovim_arch::sys::gettid())
    }

    /// Registers one visible service row with an explicit owner thread id.
    ///
    /// The explicit form is used by no-std fixtures to exercise `SEND_SAFE`
    /// gates deterministically without spawning threads.
    ///
    /// # Errors
    ///
    /// Returns an error on duplicate service key or allocation failure.
    pub fn register_with_thread(
        &mut self,
        meta: ServiceDescriptorMeta,
        owner_generation: u64,
        owner_thread_id: i32,
    ) -> Result<(), &'static str> {
        if self.rows.get(&meta.key).is_some() {
            return Err("service: duplicate key");
        }
        self.rows
            .try_insert(meta.key, ServiceRowRecord::new(meta, owner_generation, owner_thread_id))
            .map_err(|_| "alloc")?;
        Ok(())
    }

    /// Performs a borrow-only lookup.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceAccessError::NotFound`] if no visible row exists.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn borrow(&self, key: ServiceKey) -> Result<ServiceBorrow, ServiceAccessError> {
        let row = self.visible_row(key)?;
        Ok(ServiceBorrow::new(key, row.meta.owner_cdylib_id, row.owner_generation))
    }

    /// Retains a service lease.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceAccessError::NotFound`] if no visible row exists, or
    /// [`ServiceAccessError::Busy`] if lease storage cannot grow.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn lease(
        &mut self,
        key: ServiceKey,
        acquired_at_ms: u64,
    ) -> Result<ServiceLease, ServiceAccessError> {
        let row = self.visible_row(key)?;
        let raw = self.next_lease_id;
        let next = raw.checked_add(1).ok_or(ServiceAccessError::Busy)?;
        let lease = ServiceLease::new(
            ServiceLeaseId::new(raw),
            key,
            row.meta.owner_cdylib_id,
            row.owner_generation,
            acquired_at_ms,
            self.lease_timeout_ms,
        );
        self.leases
            .try_insert(lease.id, lease)
            .map_err(|_| ServiceAccessError::Busy)?;
        self.rows
            .get_mut(&key)
            .ok_or(ServiceAccessError::NotFound)?
            .outstanding_leases += 1;
        self.next_lease_id = next;
        Ok(lease)
    }

    /// Releases a retained service lease.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn release_lease(&mut self, id: ServiceLeaseId) -> bool {
        let Some(lease) = self.leases.remove(&id) else {
            return false;
        };
        if let Some(row) = self.rows.get_mut(&lease.key) {
            row.outstanding_leases = row.outstanding_leases.saturating_sub(1);
            if row.outstanding_leases == 0 && row.state == ServiceRowState::DrainingHidden {
                row.state = ServiceRowState::Revoked;
            }
        }
        true
    }

    /// Validates a retained lease against timeout, row state, and generation.
    ///
    /// # Errors
    ///
    /// Returns `Busy` on timeout and `Stale` after revocation/generation change.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn validate_lease(
        &self,
        id: ServiceLeaseId,
        now_ms: u64,
    ) -> Result<(), ServiceAccessError> {
        let lease = self.leases.get(&id).ok_or(ServiceAccessError::Stale)?;
        if lease.is_expired(now_ms) {
            return Err(ServiceAccessError::Busy);
        }
        let row = self.rows.get(&lease.key).ok_or(ServiceAccessError::Stale)?;
        if !row.state.is_visible() || !lease.matches_generation(row.owner_generation) {
            return Err(ServiceAccessError::Stale);
        }
        Ok(())
    }

    /// Enters a service call after enforcing row visibility and flags.
    ///
    /// # Errors
    ///
    /// Returns `InvalidState` when a `!SEND_SAFE` row is called from a
    /// non-owner thread, `Busy` when a `!SYNC_SAFE` row already has a live call,
    /// and `NotFound`/`Stale` for hidden or generation-mismatched rows.
    pub fn begin_call(
        &mut self,
        key: ServiceKey,
        caller_thread_id: i32,
    ) -> Result<ServiceCallToken, ServiceAccessError> {
        let row = self.visible_row_mut(key)?;
        if !row.meta.flags.send_safe() && caller_thread_id != row.owner_thread_id {
            return Err(ServiceAccessError::InvalidState);
        }
        if !row.meta.flags.sync_safe() && row.active_calls > 0 {
            return Err(ServiceAccessError::Busy);
        }
        row.active_calls = row.active_calls.saturating_add(1);
        Ok(ServiceCallToken::new(key, row.owner_generation))
    }

    /// Exits a service call token returned by [`Self::begin_call`].
    ///
    /// Returns `false` if the row is missing or the generation no longer
    /// matches the token.
    pub fn end_call(&mut self, token: ServiceCallToken) -> bool {
        let Some(row) = self.rows.get_mut(&token.key) else {
            return false;
        };
        if row.owner_generation != token.owner_generation {
            return false;
        }
        row.active_calls = row.active_calls.saturating_sub(1);
        true
    }

    /// Hides or revokes all rows owned by `owner_cdylib_id`.
    ///
    /// Rows with outstanding leases move to `DrainingHidden`; rows without
    /// leases move directly to `Revoked`.
    ///
    /// # Errors
    ///
    /// Returns `Err("alloc")` if the temporary key list cannot grow.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    pub fn revoke_owner(
        &mut self,
        owner_cdylib_id: CdylibId,
        next_generation: u64,
    ) -> Result<usize, &'static str> {
        let mut keys = Seq::new();
        for (key, row) in &self.rows {
            if row.meta.owner_cdylib_id == owner_cdylib_id {
                keys.try_push(*key).map_err(|_| "alloc")?;
            }
        }
        let mut revoked = 0usize;
        for key in keys.as_slice() {
            if let Some(row) = self.rows.get_mut(key) {
                row.owner_generation = next_generation;
                row.state = if row.outstanding_leases == 0 {
                    ServiceRowState::Revoked
                } else {
                    ServiceRowState::DrainingHidden
                };
                revoked += 1;
            }
        }
        Ok(revoked)
    }

    /// Returns a row by key.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub fn row(&self, key: ServiceKey) -> Option<&ServiceRowRecord> {
        self.rows.get(&key)
    }

    fn visible_row(&self, key: ServiceKey) -> Result<&ServiceRowRecord, ServiceAccessError> {
        let row = self.rows.get(&key).ok_or(ServiceAccessError::NotFound)?;
        if row.state.is_visible() {
            Ok(row)
        } else {
            Err(ServiceAccessError::NotFound)
        }
    }

    fn visible_row_mut(
        &mut self,
        key: ServiceKey,
    ) -> Result<&mut ServiceRowRecord, ServiceAccessError> {
        let row = self
            .rows
            .get_mut(&key)
            .ok_or(ServiceAccessError::NotFound)?;
        if row.state.is_visible() {
            Ok(row)
        } else {
            Err(ServiceAccessError::NotFound)
        }
    }
}

/// Token representing an active service call gate entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServiceCallToken {
    /// Service key whose row was entered.
    pub key: ServiceKey,
    /// Owner generation captured at entry.
    pub owner_generation: u64,
}

impl ServiceCallToken {
    /// Builds a service call token.
    #[must_use]
    pub const fn new(key: ServiceKey, owner_generation: u64) -> Self {
        Self {
            key,
            owner_generation,
        }
    }
}

/// One service row retained by the kernel registry.
///
/// ```rust,no_run
/// // no_run: requires arch allocator runtime.
/// ```
pub struct ServiceRowRecord {
    /// Descriptor metadata.
    pub meta: ServiceDescriptorMeta,
    /// Visible/draining/revoked lifecycle state.
    pub state: ServiceRowState,
    /// Owner generation captured by leases and borrows.
    pub owner_generation: u64,
    /// Outstanding lease count.
    pub outstanding_leases: u32,
    /// Thread that registered this service row.
    pub owner_thread_id: i32,
    /// Live service calls currently inside the row.
    pub active_calls: u32,
}

impl ServiceRowRecord {
    /// Builds a visible service row.
    ///
    /// ```rust,no_run
    /// // no_run: requires arch allocator runtime.
    /// ```
    #[must_use]
    pub const fn new(
        meta: ServiceDescriptorMeta,
        owner_generation: u64,
        owner_thread_id: i32,
    ) -> Self {
        Self {
            meta,
            state: ServiceRowState::Visible,
            owner_generation,
            outstanding_leases: 0,
            owner_thread_id,
            active_calls: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RegisterAddress {
    key: RegisterKey,
    client_id: ClientId,
    session_id: SessionId,
}

impl RegisterAddress {
    const fn new(
        scope: RegisterScope,
        client_id: ClientId,
        session_id: SessionId,
        id: RegisterId,
    ) -> Self {
        let scoped_client = match scope {
            RegisterScope::Client => client_id,
            RegisterScope::Session | RegisterScope::System => ClientId::new(0),
        };
        let scoped_session = match scope {
            RegisterScope::Client | RegisterScope::Session => session_id,
            RegisterScope::System => SessionId::new(0),
        };
        Self {
            key: RegisterKey::new(scope, id),
            client_id: scoped_client,
            session_id: scoped_session,
        }
    }

    fn is_same_limit_bucket(self, other: Self) -> bool {
        if self.key.scope != other.key.scope {
            return false;
        }
        match self.key.scope {
            RegisterScope::Client => {
                self.client_id.as_u32() == other.client_id.as_u32()
                    && self.session_id.as_u32() == other.session_id.as_u32()
            }
            RegisterScope::Session => self.session_id.as_u32() == other.session_id.as_u32(),
            RegisterScope::System => true,
        }
    }
}

const fn records_undo_group(origin: EditOrigin) -> bool {
    origin.participates_in_undo() || matches!(origin, EditOrigin::External)
}

const fn validate_range(bytes: &Bytes, start: usize, end: usize) -> Result<(), &'static str> {
    if start > end || end > bytes.len() {
        Err("buffer: invalid byte range")
    } else {
        Ok(())
    }
}

fn replace_range_bytes(
    source: &Bytes,
    start: usize,
    end: usize,
    replacement: &[u8],
) -> Result<Bytes, &'static str> {
    validate_range(source, start, end)?;
    let mut out = Bytes::new();
    out.try_extend_from_slice(&source.as_slice()[..start])
        .map_err(|_| "alloc")?;
    out.try_extend_from_slice(replacement)
        .map_err(|_| "alloc")?;
    out.try_extend_from_slice(&source.as_slice()[end..])
        .map_err(|_| "alloc")?;
    Ok(out)
}

fn apply_group_undo(source: &Bytes, group: &UndoGroup) -> Result<Bytes, &'static str> {
    let mut current = clone_bytes(source)?;
    let edits = group.edits.as_slice();
    let mut idx = edits.len();
    while idx > 0 {
        idx -= 1;
        let edit = &edits[idx];
        let end = edit
            .start
            .checked_add(edit.new_bytes.len())
            .ok_or("buffer: invalid byte range")?;
        current = replace_range_bytes(&current, edit.start, end, edit.old_bytes.as_slice())?;
    }
    Ok(current)
}

fn apply_group_redo(source: &Bytes, group: &UndoGroup) -> Result<Bytes, &'static str> {
    let mut current = clone_bytes(source)?;
    for edit in group.edits.as_slice() {
        current = replace_range_bytes(&current, edit.start, edit.end, edit.new_bytes.as_slice())?;
    }
    Ok(current)
}

fn find_group_mut(groups: &mut Seq<UndoGroup>, id: UndoGroupId) -> Option<&mut UndoGroup> {
    groups
        .as_mut_slice()
        .iter_mut()
        .find(|group| group.id == id)
}

fn clear_seq<T>(seq: &mut Seq<T>) {
    while seq.pop().is_some() {}
}

fn remove_first_group(groups: &mut Seq<UndoGroup>) -> bool {
    if groups.is_empty() {
        return false;
    }
    let mut idx = 0usize;
    while idx + 1 < groups.len() {
        groups.as_mut_slice().swap(idx, idx + 1);
        idx += 1;
    }
    groups.pop().is_some()
}

fn undo_recorded_bytes(stack: &UndoStack) -> usize {
    undo_group_bytes(&stack.groups) + undo_group_bytes(&stack.redo_groups)
}

fn undo_group_bytes(groups: &Seq<UndoGroup>) -> usize {
    let mut bytes = 0usize;
    for group in groups.as_slice() {
        for edit in group.edits.as_slice() {
            bytes = bytes
                .saturating_add(edit.old_bytes.len())
                .saturating_add(edit.new_bytes.len());
        }
    }
    bytes
}

fn clone_bytes(bytes: &Bytes) -> Result<Bytes, &'static str> {
    clone_bytes_slice(bytes.as_slice())
}

fn clone_bytes_slice(bytes: &[u8]) -> Result<Bytes, &'static str> {
    Bytes::try_from_slice(bytes).map_err(|_| "alloc")
}

fn clone_cursor_carrier(carrier: &CursorCarrier) -> Result<CursorCarrier, &'static str> {
    Ok(CursorCarrier::new(carrier.header, clone_bytes(&carrier.content)?))
}

// L12 layout: tests in sibling state_tests.rs, declared in lib.rs.
