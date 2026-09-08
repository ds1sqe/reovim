//! Two concrete local selection policies; no Kernel Position or cursor state.

use reovim_kernel_interface::buffer::{
    BufferHistoryDirection, BufferStateStamp,
};
use reovim_kernel_interface::interaction::{
    notification::BufferChangeNotification,
    session::{InteractionSessionHandle, RepresentationAttachmentHandle},
};
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;

use super::cases::{ReceivedSnapshot, keyed_observation};
use crate::first_edit::native::models::{Key, KeyedState};

mod receive;
mod selection;

use receive::Snapshot;
use selection::{AlphaSelection, KeySelection, Translation};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Response {
    Follow,
    Keep,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Pressure,
    StaleState,
    StaleProjection,
    SnapshotMismatch,
    InvalidSnapshot,
    WrongSchema,
    Unrepresentable,
    PartialNeedsConsent,
    WrongObserver,
}

/// Fixed one-notice/one-snapshot rendezvous, not a general client runtime.
struct Client {
    session: InteractionSessionHandle,
    attachment: RepresentationAttachmentHandle,
    observed: BufferStateStamp,
    projection: ProjectionStamp,
    selection: KeySelection,
    response: Response,
    notice: Option<BufferChangeNotification>,
    snapshot: Option<Snapshot>,
    last_notice: Option<BufferChangeNotification>,
    values: Option<KeyedState>,
    alpha_view: Option<AlphaSelection>,
    selection_failure: Option<Failure>,
}

impl Client {
    fn new(
        received: &ReceivedSnapshot,
        before: &BufferStateStamp,
        response: Response,
    ) -> Self {
        Self {
            session: received.session.stamp().handle(),
            attachment: received.attachment.handle(),
            observed: *before,
            projection: received.previous_projection,
            selection: KeySelection::disjoint(&received.attachment, before),
            response,
            notice: None,
            snapshot: None,
            last_notice: None,
            values: None,
            alpha_view: None,
            selection_failure: None,
        }
    }

    fn receive_notice(
        &mut self,
        notice: BufferChangeNotification,
    ) -> Result<bool, Failure> {
        if self.notice.is_some() {
            return Err(Failure::Pressure);
        }
        if !notice.before().same_state(self.observed) {
            return Err(Failure::StaleState);
        }
        self.notice = Some(notice);
        self.install_if_paired()
    }

    fn install_if_paired(&mut self) -> Result<bool, Failure> {
        let (Some(notice), Some(snapshot)) = (&self.notice, &self.snapshot)
        else {
            return Ok(false);
        };
        if !snapshot.source.same_state(notice.accepted().result()) {
            return Err(Failure::SnapshotMismatch);
        }
        if self.response == Response::Follow {
            self.selection_failure = self
                .selection
                .follow(&self.observed, &snapshot.source, &snapshot.values)
                .err();
        }
        // Snapshot acceptance is independent of whether local selection maps.
        // A kept/unmappable value retains its old stamp; it is not relabeled.
        self.observed = snapshot.source;
        self.projection = snapshot.projection;
        self.values = self.snapshot.take().map(|value| value.values);
        self.last_notice = self.notice.take();
        Ok(true)
    }

    fn edit_selection(&self) -> Result<&KeySelection, Failure> {
        self.selection.check_current(&self.observed)?;
        Ok(&self.selection)
    }

    fn select_alpha(
        &mut self,
        translated: Translation,
        allow_partial: bool,
    ) -> Result<(), Failure> {
        let selected = translation_choice(translated, allow_partial)?;
        if !selected.source.same_state(self.observed) {
            return Err(Failure::StaleState);
        }
        self.alpha_view = Some(selected);
        Ok(())
    }
}

fn translation_choice(
    translated: Translation,
    allow_partial: bool,
) -> Result<AlphaSelection, Failure> {
    match translated {
        Translation::Exact(value) => Ok(value),
        Translation::Partial { selected, omitted } => {
            if !allow_partial {
                return Err(Failure::PartialNeedsConsent);
            }
            assert_eq!(omitted, [Key::Omega].into());
            Ok(selected)
        }
        Translation::Unrepresentable => Err(Failure::Unrepresentable),
    }
}

/// Observes one real fixture transition through two independent local policies.
///
/// # Panics
///
/// Panics if receipt ordering, explicit mapping or local independence fails.
#[must_use]
pub fn exercise_client_responses() -> bool {
    let mut pair = keyed_observation();
    let mut follower = Client::new(&pair.first, &pair.before, Response::Follow);
    let mut keeper = Client::new(&pair.second, &pair.before, Response::Keep);
    keeper.selection.keys = [Key::Omega].into();
    let original = follower.selection.clone();
    assert_eq!(
        pair.notice.accepted().history_direction(),
        Some(BufferHistoryDirection::Undo),
    );
    assert_eq!(original.keys, [Key::Alpha, Key::Omega].into());
    assert!(!follower.receive_notice(pair.notice.clone()).unwrap());
    assert!(!keeper.receive_snapshot(&pair.second).unwrap());
    assert_eq!(follower.selection, original);
    assert_eq!(keeper.selection.source, pair.before);
    assert!(follower.receive_snapshot(&pair.first).unwrap());
    assert!(keeper.receive_notice(pair.notice.clone()).unwrap());
    independent_results(&follower, &keeper, &pair.notice);
    translation_choices(&mut follower, &keeper);
    super::cases::finish(&mut pair.owner);
    true
}

fn independent_results(
    follower: &Client,
    keeper: &Client,
    notice: &BufferChangeNotification,
) {
    let result = notice.accepted().result();
    assert_eq!(follower.observed, result);
    assert_eq!(keeper.observed, result);
    assert_eq!(follower.last_notice.as_ref(), Some(notice));
    assert_eq!(keeper.last_notice.as_ref(), Some(notice));
    assert_eq!(
        follower.values,
        Some([(Key::Alpha, false), (Key::Omega, true)].into())
    );
    assert_eq!(keeper.values, follower.values);
    assert_eq!(follower.edit_selection().unwrap().source, result);
    assert_eq!(keeper.edit_selection(), Err(Failure::StaleState));
    assert_eq!(follower.selection.keys, [Key::Alpha, Key::Omega].into());
    assert_eq!(keeper.selection.keys, [Key::Omega].into());
    assert_eq!(follower.selection_failure, None);
}

fn translation_choices(follower: &mut Client, keeper: &Client) {
    let original = follower.selection.clone();
    let partial = original.to_alpha(&follower.observed).unwrap();
    assert_eq!(
        follower.select_alpha(partial, false),
        Err(Failure::PartialNeedsConsent),
    );
    assert_eq!(follower.alpha_view, None);
    assert_eq!(follower.selection, original);
    let partial = original.to_alpha(&follower.observed).unwrap();
    assert_eq!(follower.select_alpha(partial, true), Ok(()));
    let selected = follower.alpha_view.as_ref().unwrap();
    assert_eq!(selected.schema, selection::alpha_schema());
    assert_eq!(selected.source, follower.observed);
    let mut exact = original.clone();
    exact.keys = [Key::Alpha].into();
    assert!(matches!(
        exact.to_alpha(&follower.observed),
        Ok(Translation::Exact(_))
    ));
    let mut missing = original.clone();
    missing.keys = [Key::Omega].into();
    assert_eq!(
        missing.to_alpha(&follower.observed),
        Ok(Translation::Unrepresentable)
    );
    assert_eq!(
        keeper.selection.to_alpha(&keeper.observed),
        Err(Failure::StaleState)
    );
    // Translation failure/partial consent never changes the accepted revision.
    assert_eq!(follower.observed, keeper.observed);
    assert_eq!(follower.selection, original);
}
