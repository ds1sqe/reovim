mod derives;
mod intent;
mod provider;
mod session;
mod support;
mod update;

use crate::InteractionDebugInputs;

pub const fn const_cases() -> bool {
    derives::all()
        && session::const_cases()
        && intent::const_cases()
        && update::const_cases()
}

pub fn exercise() -> bool {
    support::descriptor_cases()
        && session::exercise()
        && intent::exercise()
        && update::exercise()
        && provider::exercise()
}

pub fn debug_inputs() -> Option<InteractionDebugInputs> {
    let fixture = support::fixture()?;
    let intent = support::make_intent(&fixture, 5_000, 5_001, 5_002)?;
    let initial = support::initial_update(&fixture)?;
    let delta = support::delta_update(&fixture)?;
    Some(InteractionDebugInputs {
        session: fixture.session,
        attachment: fixture.attachment,
        intent,
        initial,
        delta,
    })
}
