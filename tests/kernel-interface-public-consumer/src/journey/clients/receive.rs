use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::interaction::update::RepresentationUpdateLineage;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::{Client, Failure, Key, KeyedState, ReceivedSnapshot};

pub struct Snapshot {
    pub source: BufferStateStamp,
    pub projection: ProjectionStamp,
    pub values: KeyedState,
}

impl Client {
    pub fn receive_snapshot(
        &mut self,
        received: &ReceivedSnapshot,
    ) -> Result<bool, Failure> {
        if received.session.stamp().handle() != self.session
            || received.attachment.handle() != self.attachment
        {
            return Err(Failure::WrongObserver);
        }
        if self.snapshot.is_some() {
            return Err(Failure::Pressure);
        }
        self.snapshot = Some(decode(received, &self.projection)?);
        self.install_if_paired()
    }
}

fn decode(
    received: &ReceivedSnapshot,
    current: &ProjectionStamp,
) -> Result<Snapshot, Failure> {
    received
        .update
        .validate(&received.session, &received.attachment)
        .map_err(|_| Failure::InvalidSnapshot)?;
    let before = received
        .update
        .stamp()
        .resolve_projection(&received.session, &received.attachment)
        .map_err(|_| Failure::StaleProjection)?;
    if !before.same_state(*current) {
        return Err(Failure::StaleProjection);
    }
    let RepresentationUpdateLineage::FullSnapshot { supersedes, result } =
        received.update.lineage()
    else {
        return Err(Failure::InvalidSnapshot);
    };
    if supersedes != current.predecessor() {
        return Err(Failure::StaleProjection);
    }
    let source = received
        .update
        .stamp()
        .resolve_target(&received.session, &received.attachment)
        .map_err(|_| Failure::InvalidSnapshot)?;
    let carrier = received.update.carrier().ok_or(Failure::InvalidSnapshot)?;
    let values = keyed_values(carrier.bytes())?;
    let projection = ProjectionStamp::new(
        current.projection(),
        current.lifecycle(),
        current.representation(),
        result,
    );
    Ok(Snapshot { source, projection, values })
}

fn keyed_values(bytes: &[u8]) -> Result<KeyedState, Failure> {
    let [b'K', 17, alpha, 93, omega] = bytes else {
        return Err(Failure::InvalidSnapshot);
    };
    Ok([(Key::Alpha, boolean(*alpha)?), (Key::Omega, boolean(*omega)?)].into())
}

const fn boolean(value: u8) -> Result<bool, Failure> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Failure::InvalidSnapshot),
    }
}
