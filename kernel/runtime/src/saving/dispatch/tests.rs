use super::{Error, Status, refine};

#[test]
fn refinement_preserves_partial_facts_and_atomicity_violations() {
    let partial = Status::PartiallyApplied {
        cause: Error::UNAVAILABLE.code,
        atomicity_violated: false,
    };
    let violated = Status::PartiallyApplied {
        cause: Error::UNAVAILABLE.code,
        atomicity_violated: true,
    };
    assert_eq!(refine(Some(partial), Status::Applied), Ok(Status::Applied));
    assert_eq!(
        refine(Some(partial), Status::OutcomeUnknown(Error::UNAVAILABLE.code)),
        Err(Error::PROVIDER)
    );
    assert_eq!(
        refine(Some(partial), Status::DefinitelyNotApplied(Error::STALE.code)),
        Err(Error::PROVIDER)
    );
    assert_eq!(refine(Some(violated), Status::Applied), Err(Error::PROVIDER));
    assert_eq!(refine(Some(violated), partial), Err(Error::PROVIDER));
    assert_eq!(refine(Some(violated), violated), Ok(violated));
}
