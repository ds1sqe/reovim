use reovim_kernel_interface::buffer::BufferStateStamp;
use reovim_kernel_interface::interaction::intent::InteractionIntent;
use reovim_kernel_interface::semantic::stamp::ProjectionStamp;
use reovim_resource_local::Key;

use super::{
    Binding, KeySet, QualifiedSelection, ResponsePolicy, SelectionError,
    SelectionStatus, Value, heading_schema, keyed_schema,
};
use crate::client::{Client, ClientError};

impl Client {
    /// Selects disjoint native keys under the actual observed keyed snapshot.
    ///
    /// # Errors
    /// Rejects unsupported schemas, missing keys or unavailable observation.
    pub fn select_keys(
        &mut self,
        keys: KeySet,
        response: ResponsePolicy,
    ) -> Result<(), SelectionError> {
        let binding = self.selection_binding()?;
        let selection = QualifiedSelection {
            schema: keyed_schema(),
            binding,
            value: Value::Keys(keys),
        };
        self.validate_selection_value(&selection)?;
        self.selection = Some(selection);
        self.response = response;
        self.selection_issue = None;
        Ok(())
    }

    /// Selects the first profile's heading-label meaning.
    ///
    /// # Errors
    /// Rejects unavailable or unsupported heading correspondence.
    pub fn select_heading_label(
        &mut self,
        response: ResponsePolicy,
    ) -> Result<(), SelectionError> {
        let selection = QualifiedSelection {
            schema: heading_schema(),
            binding: self.selection_binding()?,
            value: Value::HeadingLabel,
        };
        self.validate_selection_value(&selection)?;
        self.selection = Some(selection);
        self.response = response;
        self.selection_issue = None;
        Ok(())
    }

    /// Borrows local state separately from the currently observed snapshot.
    #[must_use]
    pub const fn selection(&self) -> Option<&QualifiedSelection> {
        self.selection.as_ref()
    }

    /// Reports stale kept values instead of silently treating them as current.
    #[must_use]
    pub fn selection_status(&self) -> Option<SelectionStatus> {
        let selected = self.selection.as_ref()?;
        if self.selection_issue == Some(SelectionError::Unrepresentable) {
            return Some(SelectionStatus::Unrepresentable);
        }
        let current = self.selection_binding().is_ok_and(|binding| {
            selected.binding == binding && self.selection_issue.is_none()
        });
        Some(if current {
            SelectionStatus::Current
        } else {
            SelectionStatus::Stale
        })
    }

    /// Resolves stable native identities explicitly against the current view.
    ///
    /// # Errors
    /// Preserves the prior selection when correspondence fails.
    pub fn revalidate_selection(&mut self) -> Result<(), SelectionError> {
        let selected =
            self.selection.as_ref().ok_or(SelectionError::Missing)?;
        let binding = self.selection_binding()?;
        if !same_family(&selected.binding, &binding) {
            return Err(SelectionError::Stale);
        }
        self.validate_selection_value(selected)?;
        let mut next = selected.clone();
        next.binding = binding;
        self.selection = Some(next);
        self.selection_issue = None;
        Ok(())
    }

    /// Builds an edit only if its chosen part has current local correspondence.
    /// Direct explicit semantic `intent` remains independent of this policy.
    ///
    /// # Errors
    /// Rejects stale selection, unselected parts or invalid semantic encoding.
    pub fn selection_intent(
        &self,
        request: u128,
        payload: &[u8],
    ) -> Result<InteractionIntent, ClientError> {
        if self.selection_status() != Some(SelectionStatus::Current) {
            return Err(ClientError::Selection(SelectionError::Stale));
        }
        let selected = self
            .selection
            .as_ref()
            .ok_or(ClientError::Selection(SelectionError::Missing))?;
        validate_action(selected, payload).map_err(ClientError::Selection)?;
        self.intent(request, payload)
    }

    pub(in crate::client) fn follow_selection(
        &mut self,
        prior: &BufferStateStamp,
        projection: &ProjectionStamp,
    ) {
        if self.response != ResponsePolicy::Follow || self.selection.is_none() {
            return;
        }
        let current = self.selection.as_ref().is_some_and(|selected| {
            selected.binding.source == *prior
                && selected.binding.projection == *projection
        });
        if !current {
            self.selection_issue = Some(SelectionError::Stale);
            return;
        }
        self.selection_issue = self.revalidate_selection().err();
    }

    pub(super) fn selection_binding(&self) -> Result<Binding, SelectionError> {
        if self.payload.is_none() {
            return Err(SelectionError::Stale);
        }
        Ok(Binding {
            source: self.state.ok_or(SelectionError::Stale)?,
            model: *self.policy.state_model(),
            representation: self.policy.stamp(),
            coordinate: self.attachment.coordinate(),
            projection: self.projection,
        })
    }

    pub(super) fn validate_selection_value(
        &self,
        selected: &QualifiedSelection,
    ) -> Result<(), SelectionError> {
        let bytes = self.payload().ok_or(SelectionError::Stale)?;
        match selected.value {
            Value::Keys(keys) => validate_keys(self, bytes, keys),
            Value::HeadingLabel => validate_heading(self, bytes),
        }
    }
}

fn same_family(left: &Binding, right: &Binding) -> bool {
    left.model == right.model
        && left.representation == right.representation
        && left.coordinate == right.coordinate
        && left.source.buffer() == right.source.buffer()
        && left.source.source().access() == right.source.source().access()
        && left.projection.projection() == right.projection.projection()
        && left.projection.lifecycle() == right.projection.lifecycle()
}

fn validate_keys(
    client: &Client,
    bytes: &[u8],
    keys: KeySet,
) -> Result<(), SelectionError> {
    if client.session.update_contract().schema().identity() != 0x7008 {
        return Err(SelectionError::Unsupported);
    }
    let [b'K', records @ ..] = bytes else {
        return Err(SelectionError::InvalidSnapshot);
    };
    if records.len() > 4 || !records.len().is_multiple_of(2) {
        return Err(SelectionError::InvalidSnapshot);
    }
    let mut present = KeySet { alpha: false, omega: false };
    for [tag, enabled] in records.as_chunks::<2>().0 {
        add_key(&mut present, *tag, *enabled)?;
    }
    if !keys.is_subset_of(present) {
        return Err(SelectionError::Unrepresentable);
    }
    Ok(())
}

fn add_key(
    keys: &mut KeySet,
    tag: u8,
    enabled: u8,
) -> Result<(), SelectionError> {
    let key = Key::from_tag(tag).ok_or(SelectionError::InvalidSnapshot)?;
    if enabled > 1 || keys.contains(key) {
        return Err(SelectionError::InvalidSnapshot);
    }
    match key {
        Key::Alpha => keys.alpha = true,
        Key::Omega => keys.omega = true,
    }
    Ok(())
}

const fn validate_heading(
    client: &Client,
    bytes: &[u8],
) -> Result<(), SelectionError> {
    if client.session.update_contract().schema().identity() != 0x6008 {
        return Err(SelectionError::Unsupported);
    }
    match bytes {
        [b'H', 1 | 2, label] if label.is_ascii_alphabetic() => Ok(()),
        _ => Err(SelectionError::Unrepresentable),
    }
}

fn validate_action(
    selected: &QualifiedSelection,
    payload: &[u8],
) -> Result<(), SelectionError> {
    let Value::Keys(keys) = selected.value else {
        return Ok(());
    };
    let [tag] = payload else {
        return Err(SelectionError::OutsideSelection);
    };
    let key = Key::from_tag(*tag).ok_or(SelectionError::OutsideSelection)?;
    if !keys.contains(key) {
        return Err(SelectionError::OutsideSelection);
    }
    Ok(())
}
