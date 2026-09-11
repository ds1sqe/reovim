//! Ordinary consumers import Interface; assembly supplies granted access.

use reovim_kernel_interface::{
    Connection, Contract, Message, RequestError,
    buffer::{self, Observation},
};

/// # Errors
/// Preserves ordinary Resource admission, execution and recovery errors.
// Connections belong to one execution thread in this ABI profile.
#[allow(clippy::future_not_send)]
pub async fn heading(kernel: &Connection) -> Result<Observation, RequestError> {
    let source = kernel.resolve("resource/notes").await?;
    let original = kernel
        .read(source, &Message::new(Contract::new("resource.bytes", 1), &[]))
        .await?;
    let factory = kernel.resolve("models/heading").await?;
    let created = kernel
        .write(
            factory,
            &Message::new(Contract::new("heading.initial", 1), &original.data),
        )
        .await?;
    let before = buffer::observation(created).map_err(RequestError::Failed)?;
    let input = buffer::edit(
        before.state,
        Message::new(Contract::new("heading.replace", 1), b"b"),
    );
    let edited = kernel.write(before.state.buffer, &input).await?;
    buffer::observation(edited).map_err(RequestError::Failed)
}
