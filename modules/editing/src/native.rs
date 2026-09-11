//! Native declaration over the same pure heading model operations.

reovim_module::rv_module! {
    pub HEADING {
        contract: ("heading.model", 1),
        model: {
            initialize ("heading.initial", 1) =>
                reovim_representation_basic::heading_initial,
            prepare ("heading.replace", 1) =>
                reovim_representation_basic::heading_edit,
            observe ("heading.source", 1) =>
                reovim_representation_basic::heading_read,
        },
    }
}
