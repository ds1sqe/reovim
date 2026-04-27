use super::*;

#[test]
fn expander_impl_implements_trait() {
    let expander = SnippetExpanderImpl;
    let _: &dyn SnippetExpander = &expander;
}
