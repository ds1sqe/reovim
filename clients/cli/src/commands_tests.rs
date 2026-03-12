use super::*;

// Tests that don't require a running server

#[test]
fn test_output_format_eq() {
    assert_eq!(OutputFormat::Plain, OutputFormat::Plain);
    assert_eq!(OutputFormat::Json, OutputFormat::Json);
    assert_ne!(OutputFormat::Plain, OutputFormat::Json);
}
