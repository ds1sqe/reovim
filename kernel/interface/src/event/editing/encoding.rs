use core::fmt::{self, Write};

use crate::buffer::BufferStateStamp;
use crate::lifecycle::LifecycleIdentity;

use super::{EDITING_CANONICAL_MAX_BYTES, EditingRecord, EditingRecordError};

#[derive(Clone)]
pub(super) struct Bytes {
    storage: [u8; EDITING_CANONICAL_MAX_BYTES],
    used: usize,
}

impl Bytes {
    pub const fn new() -> Self {
        Self { storage: [0; EDITING_CANONICAL_MAX_BYTES], used: 0 }
    }
    pub const fn len(&self) -> usize {
        self.used
    }
    pub fn as_slice(&self) -> &[u8] {
        &self.storage[..self.used]
    }
}

impl Write for Bytes {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.used.checked_add(text.len()).ok_or(fmt::Error)?;
        let target = self.storage.get_mut(self.used..end).ok_or(fmt::Error)?;
        target.copy_from_slice(text.as_bytes());
        self.used = end;
        Ok(())
    }
}

pub(super) fn render(
    record: &EditingRecord,
) -> Result<Bytes, EditingRecordError> {
    let mut bytes = Bytes::new();
    render_fields(&mut bytes, record)
        .map_err(|_| EditingRecordError::EncodingBound)?;
    Ok(bytes)
}

fn render_fields(bytes: &mut Bytes, record: &EditingRecord) -> fmt::Result {
    bytes.write_str("kernel.editing/v2 stream=")?;
    life(bytes, record.stream)?;
    write!(bytes, " seq={} producer=", record.sequence.value())?;
    life(bytes, record.producer)?;
    bytes.write_str(" clock=")?;
    life(bytes, record.clock)?;
    write!(
        bytes,
        " ns={} severity=info stage={} session=",
        record.nanoseconds,
        record.stage.token()
    )?;
    session(bytes, record.context.session)?;
    bytes.write_str(" observer=")?;
    session(bytes, record.context.observer)?;
    bytes.write_str(" update=")?;
    update(bytes, record.context.update.as_ref())?;
    bytes.write_str(" request=")?;
    optional(
        bytes,
        record.context.request.map(
            crate::interaction::identity::InteractionRequestIdentity::value,
        ),
    )?;
    bytes.write_str(" correlation=")?;
    optional(
        bytes,
        record
            .context
            .correlation
            .map(crate::buffer::plan::BufferCorrelationIdentity::value),
    )?;
    bytes.write_str(" parent=")?;
    optional(
        bytes,
        record.context.parent.map(|parent| u128::from(parent.value())),
    )?;
    bytes.write_str(" additional-parent=")?;
    optional(
        bytes,
        record
            .context
            .additional_parent
            .map(|parent| u128::from(parent.value())),
    )?;
    bytes.write_str(" state=")?;
    state(bytes, record.context.state.as_ref())?;
    bytes.write_str(" cause=")?;
    match record.context.cause {
        Some(cause) => write!(bytes, "{cause:?}")?,
        None => bytes.write_char('-')?,
    }
    bytes.write_char('\n')
}

fn update(
    bytes: &mut Bytes,
    value: Option<
        &crate::interaction::update::RepresentationUpdateIdentityStamp,
    >,
) -> fmt::Result {
    let Some(stamp) = value else { return bytes.write_char('-') };
    let stream = stamp.stream();
    write!(bytes, "{:x}@", stream.stream().value())?;
    life(bytes, stream.lifecycle())?;
    write!(bytes, "/{:x}/{}", stamp.update().value(), stamp.cursor().value())
}

fn life(bytes: &mut Bytes, stamp: LifecycleIdentity) -> fmt::Result {
    write!(
        bytes,
        "{:x}:{:x}:{}",
        stamp.instance(),
        stamp.incarnation(),
        stamp.generation().value()
    )
}

fn optional(bytes: &mut Bytes, value: Option<u128>) -> fmt::Result {
    match value {
        Some(value) => write!(bytes, "{value:x}"),
        None => bytes.write_char('-'),
    }
}

fn session(
    bytes: &mut Bytes,
    value: Option<crate::interaction::session::InteractionSessionHandle>,
) -> fmt::Result {
    let Some(stamp) = value else { return bytes.write_char('-') };
    write!(bytes, "{:x}@", stamp.session().value())?;
    life(bytes, stamp.lifecycle())
}

fn state(bytes: &mut Bytes, value: Option<&BufferStateStamp>) -> fmt::Result {
    let Some(stamp) = value else { return bytes.write_char('-') };
    let source = stamp.source();
    write!(bytes, "{:x}@", source.access().resource().value())?;
    life(bytes, source.access().lifecycle())?;
    let schema = source.revision().schema();
    write!(
        bytes,
        "/{:x}/{:x}:{}:{}:{}/{:x}/{:x}@",
        source.snapshot().value(),
        schema.identity(),
        schema.selected(),
        schema.minimum(),
        schema.maximum(),
        source.revision().token(),
        stamp.buffer().buffer().value()
    )?;
    life(bytes, stamp.buffer().lifecycle())?;
    write!(
        bytes,
        "/{}/{:x}@",
        stamp.version().value(),
        stamp.history().node().value()
    )?;
    life(bytes, stamp.history().lifecycle())
}
