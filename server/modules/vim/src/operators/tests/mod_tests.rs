use super::super::*;

#[test]
fn test_operators_list() {
    let ops = operators();
    assert_eq!(ops.len(), 6);

    // Check operators are present
    let ids: Vec<_> = ops.iter().map(|o| o.id()).collect();
    assert!(ids.contains(&"delete"));
    assert!(ids.contains(&"yank"));
    assert!(ids.contains(&"change"));
}

#[test]
fn test_delete_is_text_modifying() {
    let delete = DeleteOperator;
    assert!(delete.is_text_modifying());
    assert!(!delete.is_linewise());
}

#[test]
fn test_operators_ids_unique() {
    let ops = operators();
    let ids: Vec<_> = ops.iter().map(|o| o.id()).collect();
    // Check all IDs are unique
    let mut seen = std::collections::HashSet::new();
    for id in &ids {
        assert!(seen.insert(id), "Duplicate operator id: {id}");
    }
}

#[test]
fn test_operator_commands_not_empty() {
    let cmds = operator_commands();
    assert!(!cmds.is_empty());
}

#[test]
fn test_operator_commands_count() {
    let cmds = operator_commands();
    assert_eq!(cmds.len(), 6); // delete, yank, change, lowercase, uppercase, toggle-case
}

#[test]
fn test_delete_operator_debug() {
    let delete = DeleteOperator;
    assert!(format!("{delete:?}").contains("DeleteOperator"));
}

#[test]
fn test_yank_operator_debug() {
    let yank = YankOperator;
    assert!(format!("{yank:?}").contains("YankOperator"));
}

#[test]
fn test_change_operator_debug() {
    let change = ChangeOperator;
    assert!(format!("{change:?}").contains("ChangeOperator"));
}

#[test]
fn test_operators_all_not_linewise() {
    let ops = operators();
    for op in &ops {
        assert!(!op.is_linewise(), "operator '{}' should not be linewise by default", op.id());
    }
}

#[test]
fn test_operators_text_modifying_flags() {
    let ops = operators();
    let text_modifying: Vec<_> = ops
        .iter()
        .filter(|o| o.is_text_modifying())
        .map(|o| o.id())
        .collect();
    assert!(text_modifying.contains(&"delete"));
    assert!(text_modifying.contains(&"change"));
    assert!(!text_modifying.contains(&"yank"));
}

#[test]
fn test_delete_operator_clone_copy() {
    let del = DeleteOperator;
    let copied: DeleteOperator = del;
    assert_eq!(copied.id(), "delete");
}

#[test]
fn test_yank_operator_clone_copy() {
    let yank = YankOperator;
    let copied: YankOperator = yank;
    assert_eq!(copied.id(), "yank");
}

#[test]
fn test_operator_commands_has_all_ids() {
    let cmds = operator_commands();
    let ids: Vec<_> = cmds.iter().map(|c| c.id()).collect();
    assert!(ids.iter().any(|id| id.name() == "delete"));
    assert!(ids.iter().any(|id| id.name() == "yank"));
    assert!(ids.iter().any(|id| id.name() == "change"));
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_operator_commands_all_vim_module() {
    let cmds = operator_commands();
    for cmd in &cmds {
        assert_eq!(
            cmd.id().module().as_str(),
            "vim",
            "command '{}' should be in vim module",
            cmd.id().name()
        );
    }
}
