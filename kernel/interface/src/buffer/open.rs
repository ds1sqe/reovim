use crate::{Connection, Contract, Message, ResourceId, access::Reply};

/// Creates a Buffer from a prepared source with a retained base association.
///
/// This is source-backed creation, not namespace access to an existing Buffer.
/// Source READ rights suffice. Buffer editing does not write the source.
/// Saving back separately requires publication support and source authority.
/// A consuming stream without a coherent view/base is unsupported here; import
/// captured observations into model state instead of treating them as a base.
///
/// The Model must accept one READ initialization slot. Its initializer receives
/// the independent source view; `data` retains that Model's own meaning.
/// Initialization must represent that source state; transformations are edits.
///
/// The source's read-view provider must declare `resource.base` v1 alongside
/// its data surfaces. Prepare any backing I/O before invoking this operation.
/// Ordinary Model creation remains unbound. Decode the result with
/// [`super::descriptor`]; backing metadata grants no extra source operations.
#[must_use = "Adopt the Buffer descriptor or recover its opening request"]
pub fn open(
    connection: &Connection,
    model: ResourceId,
    source: ResourceId,
    data: &[u8],
) -> Reply {
    let mut input =
        Message::reference(Contract::new("kernel.buffer.open", 1), source);
    input.data = data.into();
    connection.write(model, &input)
}
