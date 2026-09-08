use reovim_kernel_interface::operation::ContractSelection;
use reovim_resource_local::Key;

use super::{
    KeySet, PartialAcceptance, QualifiedSelection, SelectionError,
    SelectionStatus, SelectionTranslation, TranslationStatus, Value,
    alpha_schema, keyed_schema,
};
use crate::client::Client;

impl Client {
    /// Computes correspondence without changing selection or Buffer state.
    #[must_use]
    pub fn translate_selection(
        &self,
        target: ContractSelection,
    ) -> SelectionTranslation {
        let mut result = SelectionTranslation {
            status: TranslationStatus::Unsupported,
            original: self.selection.clone(),
            selected: None,
            omitted: None,
        };
        if self.selection_status() != Some(SelectionStatus::Current) {
            result.status = TranslationStatus::Stale;
            return result;
        }
        if let Some(selected) = &self.selection {
            translate(selected, target, &mut result);
        }
        result
    }

    /// Applies exact correspondence or explicitly accepted omissions locally.
    ///
    /// # Errors
    /// Preserves the original selection on stale evidence or refused losses.
    pub fn accept_translation(
        &mut self,
        translation: SelectionTranslation,
        consent: PartialAcceptance,
    ) -> Result<(), SelectionError> {
        if self.selection_status() != Some(SelectionStatus::Current)
            || self.selection != translation.original
        {
            return Err(SelectionError::Stale);
        }
        check_status(translation.status, consent)?;
        let selected =
            translation.selected.ok_or(SelectionError::Unsupported)?;
        self.validate_selection_value(&selected)?;
        self.selection = Some(selected);
        self.selection_issue = None;
        Ok(())
    }
}

fn translate(
    source: &QualifiedSelection,
    target: ContractSelection,
    result: &mut SelectionTranslation,
) {
    if target == source.schema {
        result.status = TranslationStatus::Exact;
        result.selected = Some(source.clone());
        return;
    }
    if source.schema != keyed_schema() || target != alpha_schema() {
        return;
    }
    let Value::Keys(keys) = source.value else {
        return;
    };
    if !keys.contains(Key::Alpha) {
        result.status = TranslationStatus::Unrepresentable;
        return;
    }
    result.selected = Some(QualifiedSelection {
        schema: target,
        binding: source.binding.clone(),
        value: Value::Keys(KeySet::one(Key::Alpha)),
    });
    result.status = if keys.contains(Key::Omega) {
        result.omitted = Some(KeySet::one(Key::Omega));
        TranslationStatus::Partial
    } else {
        TranslationStatus::Exact
    };
}

const fn check_status(
    status: TranslationStatus,
    consent: PartialAcceptance,
) -> Result<(), SelectionError> {
    match (status, consent) {
        (TranslationStatus::Exact, _)
        | (TranslationStatus::Partial, PartialAcceptance::Accept) => Ok(()),
        (TranslationStatus::Partial, PartialAcceptance::Reject) => {
            Err(SelectionError::PartialNeedsConsent)
        }
        (TranslationStatus::Unrepresentable, _) => {
            Err(SelectionError::Unrepresentable)
        }
        (TranslationStatus::Unsupported, _) => Err(SelectionError::Unsupported),
        (TranslationStatus::Stale, _) => Err(SelectionError::Stale),
    }
}
