mod domain;
mod intent;
mod representation;
mod support;

use crate::SemanticPanicInputs;

pub const fn const_cases() -> bool {
    intent::const_cases()
        && domain::const_cases()
        && representation::const_cases()
}

pub fn exercise() -> bool {
    support::descriptor_cases()
        && intent::exercise()
        && domain::exercise()
        && representation::exercise()
}

pub fn panic_inputs() -> Option<SemanticPanicInputs> {
    let fixture = support::fixture()?;
    let intent = support::make_intent(&fixture, support::INTENT_BYTES)?;
    let domain_charge =
        fixture.domain.validation().bounds().prepare(4, 0, 0, 1).ok()?;
    let representation_charge = representation::charge(&fixture)?;
    Some(SemanticPanicInputs {
        domain: fixture.domain,
        representation: fixture.representation,
        buffer: fixture.buffer,
        intent,
        domain_charge,
        representation_charge,
    })
}
