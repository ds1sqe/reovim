//! Public-only effect-certainty value coverage.

use reovim_kernel_interface::effect::EffectCertainty;

pub const fn const_cases() -> bool {
    crate::derive_witness::<::reovim_kernel_interface::effect::EffectCertainty>(
    ) && certainty_code(EffectCertainty::Applied) == 0
        && certainty_code(EffectCertainty::DefinitelyNotApplied) == 1
        && certainty_code(EffectCertainty::OutcomeUnknown) == 2
}

const fn certainty_code(value: EffectCertainty) -> u8 {
    match value {
        EffectCertainty::Applied => 0,
        EffectCertainty::DefinitelyNotApplied => 1,
        EffectCertainty::OutcomeUnknown => 2,
    }
}
