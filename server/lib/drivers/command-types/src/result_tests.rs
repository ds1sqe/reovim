use super::*;

#[test]
fn test_command_result_success() {
    let result = CommandResult::Success;
    assert!(result.is_success());
    assert!(!result.is_error());
}

#[test]
fn test_command_result_error() {
    let result = CommandResult::error("Something went wrong");
    assert!(!result.is_success());
    assert!(result.is_error());
}

#[test]
fn test_command_result_error_direct_construction() {
    let result = CommandResult::Error("direct error".to_string());
    assert!(result.is_error());
    assert!(!result.is_success());
}

#[test]
fn test_command_result_error_factory() {
    let result = CommandResult::error("test message");
    assert_eq!(result, CommandResult::Error("test message".to_string()));
}

#[test]
fn test_command_result_error_factory_empty_message() {
    let result = CommandResult::error("");
    assert_eq!(result, CommandResult::Error(String::new()));
    assert!(result.is_error());
}

#[test]
fn test_command_result_equality() {
    assert_eq!(CommandResult::Success, CommandResult::Success);
    assert_eq!(CommandResult::Error("x".to_string()), CommandResult::Error("x".to_string()));

    assert_ne!(CommandResult::Success, CommandResult::Error("e".to_string()));
    assert_ne!(CommandResult::Error("a".to_string()), CommandResult::Error("b".to_string()));
}

#[test]
fn test_command_result_clone() {
    let results = [
        CommandResult::Success,
        CommandResult::Error("err".to_string()),
    ];
    for result in &results {
        let cloned = result.clone();
        assert_eq!(result, &cloned);
    }
}

#[test]
fn test_command_result_debug() {
    let debug_str = format!("{:?}", CommandResult::Success);
    assert_eq!(debug_str, "Success");

    let debug_str = format!("{:?}", CommandResult::Error("msg".to_string()));
    assert!(debug_str.contains("Error"));
    assert!(debug_str.contains("msg"));
}
