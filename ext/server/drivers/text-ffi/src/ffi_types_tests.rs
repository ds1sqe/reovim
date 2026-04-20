use {super::*, reovim_domain_text::Position};

// ========================================================================
// ReovimPosition tests
// ========================================================================

#[test]
fn test_position_new() {
    let pos = ReovimPosition::new(10, 20);
    assert_eq!(pos.line, 10);
    assert_eq!(pos.column, 20);
}

#[test]
fn test_position_from_kernel() {
    let kernel_pos = Position::new(42, 7);
    let ffi_pos = ReovimPosition::from(kernel_pos);
    assert_eq!(ffi_pos.line, 42);
    assert_eq!(ffi_pos.column, 7);
}

#[test]
fn test_position_to_kernel() {
    let ffi_pos = ReovimPosition::new(5, 3);
    let kernel_pos = Position::from(ffi_pos);
    assert_eq!(kernel_pos.line, 5);
    assert_eq!(kernel_pos.column, 3);
}

#[test]
fn test_position_roundtrip() {
    let original = Position::new(100, 200);
    let ffi = ReovimPosition::from(original);
    let back = Position::from(ffi);
    assert_eq!(original, back);
}

#[test]
fn test_position_size_and_alignment() {
    assert_eq!(std::mem::size_of::<ReovimPosition>(), 8);
    assert_eq!(std::mem::align_of::<ReovimPosition>(), 4);
}

#[test]
fn test_position_repr_c_layout() {
    let pos = ReovimPosition::new(1, 2);
    let ptr = (&raw const pos).cast::<u32>();
    unsafe {
        assert_eq!(*ptr, 1); // line
        assert_eq!(*ptr.add(1), 2); // column
    }
}

#[test]
fn test_position_zero() {
    let pos = ReovimPosition::new(0, 0);
    assert_eq!(pos, ReovimPosition { line: 0, column: 0 });
}

#[test]
fn test_position_max_values() {
    let pos = ReovimPosition::new(u32::MAX, u32::MAX);
    assert_eq!(pos.line, u32::MAX);
    assert_eq!(pos.column, u32::MAX);
}

#[test]
fn test_position_debug() {
    let pos = ReovimPosition::new(3, 7);
    let debug = format!("{pos:?}");
    assert!(debug.contains('3'));
    assert!(debug.contains('7'));
}

#[test]
fn test_position_clone_copy() {
    let pos = ReovimPosition::new(5, 10);
    let copied = pos;
    assert_eq!(pos, copied);
}

// ========================================================================
// ReovimStringResult tests
// ========================================================================

#[test]
fn test_string_result_ok() {
    let r = ReovimStringResult::ok(42);
    assert_eq!(r.status, crate::error::REOVIM_OK);
    assert_eq!(r.length, 42);
}

#[test]
fn test_string_result_err() {
    let r = ReovimStringResult::err(crate::error::REOVIM_ERR_NOT_FOUND);
    assert_eq!(r.status, crate::error::REOVIM_ERR_NOT_FOUND);
    assert_eq!(r.length, 0);
}

#[test]
fn test_string_result_size_and_alignment() {
    assert_eq!(std::mem::size_of::<ReovimStringResult>(), 8);
    assert_eq!(std::mem::align_of::<ReovimStringResult>(), 4);
}

#[test]
fn test_string_result_zero_length() {
    let r = ReovimStringResult::ok(0);
    assert_eq!(r.status, crate::error::REOVIM_OK);
    assert_eq!(r.length, 0);
}

#[test]
fn test_string_result_debug() {
    let r = ReovimStringResult::ok(5);
    let debug = format!("{r:?}");
    assert!(debug.contains("ReovimStringResult"));
}

#[test]
fn test_string_result_clone_copy() {
    let r = ReovimStringResult::ok(10);
    let copied = r;
    assert_eq!(r, copied);
}

// ========================================================================
// ReovimCommandArgs tests
// ========================================================================

#[test]
fn test_command_args_empty() {
    let args = ReovimCommandArgs::empty();
    assert_eq!(args.has_count, 0);
    assert_eq!(args.count, 0);
    assert_eq!(args.has_register, 0);
    assert_eq!(args.register, 0);
    assert_eq!(args.has_cursor, 0);
    assert_eq!(args.cursor, ReovimPosition::new(0, 0));
}

#[test]
fn test_command_args_size_and_alignment() {
    assert_eq!(std::mem::size_of::<ReovimCommandArgs>(), 28);
    assert_eq!(std::mem::align_of::<ReovimCommandArgs>(), 4);
}

#[test]
fn test_command_args_with_count() {
    let args = ReovimCommandArgs {
        has_count: 1,
        count: 5,
        ..ReovimCommandArgs::empty()
    };
    assert_eq!(args.has_count, 1);
    assert_eq!(args.count, 5);
}

#[test]
fn test_command_args_with_register() {
    let args = ReovimCommandArgs {
        has_register: 1,
        register: b'a',
        ..ReovimCommandArgs::empty()
    };
    assert_eq!(args.has_register, 1);
    assert_eq!(args.register, b'a');
}

#[test]
fn test_command_args_with_cursor() {
    let args = ReovimCommandArgs {
        has_cursor: 1,
        cursor: ReovimPosition::new(10, 5),
        ..ReovimCommandArgs::empty()
    };
    assert_eq!(args.has_cursor, 1);
    assert_eq!(args.cursor.line, 10);
    assert_eq!(args.cursor.column, 5);
}

#[test]
fn test_command_args_fully_populated() {
    let args = ReovimCommandArgs {
        has_count: 1,
        count: 3,
        has_register: 1,
        register: b'"',
        pad: [0; 3],
        has_cursor: 1,
        cursor: ReovimPosition::new(42, 7),
    };
    assert_eq!(args.has_count, 1);
    assert_eq!(args.count, 3);
    assert_eq!(args.has_register, 1);
    assert_eq!(args.register, b'"');
    assert_eq!(args.has_cursor, 1);
    assert_eq!(args.cursor, ReovimPosition::new(42, 7));
}

#[test]
fn test_command_args_debug() {
    let args = ReovimCommandArgs::empty();
    let debug = format!("{args:?}");
    assert!(debug.contains("ReovimCommandArgs"));
}

#[test]
fn test_command_args_clone_copy() {
    let args = ReovimCommandArgs {
        has_count: 1,
        count: 99,
        ..ReovimCommandArgs::empty()
    };
    let copied = args;
    assert_eq!(args, copied);
}

// ========================================================================
// ReovimYankType tests
// ========================================================================

#[test]
fn test_yank_type_size_and_alignment() {
    assert_eq!(std::mem::size_of::<ReovimYankType>(), 4);
    assert_eq!(std::mem::align_of::<ReovimYankType>(), 4);
}

#[test]
fn test_yank_type_discriminant_values() {
    assert_eq!(ReovimYankType::Characterwise as i32, 0);
    assert_eq!(ReovimYankType::Linewise as i32, 1);
}

#[test]
fn test_yank_type_from_kernel_characterwise() {
    use reovim_domain_text::YankType;
    let ffi = ReovimYankType::from(YankType::Characterwise);
    assert_eq!(ffi, ReovimYankType::Characterwise);
}

#[test]
fn test_yank_type_from_kernel_linewise() {
    use reovim_domain_text::YankType;
    let ffi = ReovimYankType::from(YankType::Linewise);
    assert_eq!(ffi, ReovimYankType::Linewise);
}

#[test]
fn test_yank_type_to_kernel_characterwise() {
    use reovim_domain_text::YankType;
    let kernel: YankType = ReovimYankType::Characterwise.into();
    assert_eq!(kernel, YankType::Characterwise);
}

#[test]
fn test_yank_type_to_kernel_linewise() {
    use reovim_domain_text::YankType;
    let kernel: YankType = ReovimYankType::Linewise.into();
    assert_eq!(kernel, YankType::Linewise);
}

#[test]
fn test_yank_type_roundtrip() {
    use reovim_domain_text::YankType;
    for yt in [YankType::Characterwise, YankType::Linewise] {
        let ffi = ReovimYankType::from(yt);
        let back: YankType = ffi.into();
        assert_eq!(yt, back);
    }
}

#[test]
fn test_yank_type_debug() {
    let yt = ReovimYankType::Characterwise;
    let debug = format!("{yt:?}");
    assert!(debug.contains("Characterwise"));
}

#[test]
fn test_yank_type_clone_copy() {
    let yt = ReovimYankType::Linewise;
    let copied = yt;
    assert_eq!(yt, copied);
}
