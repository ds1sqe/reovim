use super::*;

#[test]
fn auto_starter_implements_lifecycle() {
    let starter = LspAutoStarter;
    let _: &dyn LspLifecycle = &starter;
}
