use reovim_kernel_interface::{
    Error,
    declaration::{Declaration, Dispatch, Operation, ValueType},
};

fn name(bytes: &mut Vec<u8>, text: &str) {
    bytes.extend_from_slice(&u16::try_from(text.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
}

#[test]
fn declaration_identifiers_require_ascii() {
    let ascii = method(1, "read", &[0], &[0]);
    assert!(Declaration::decode(&interface(&[ascii])).is_ok());
    let unicode = method(1, "读取", &[0], &[0]);
    assert_eq!(
        Declaration::decode(&interface(&[unicode])),
        Err(Error::INVALID)
    );
    let mut record = vec![9, 1, 0];
    name(&mut record, "内容");
    record.push(0);
    assert_eq!(ValueType::decode(&record), Err(Error::INVALID));
}

fn method(command: u32, label: &str, input: &[u8], output: &[u8]) -> Vec<u8> {
    let mut bytes = vec![2]; // Ioctl.
    bytes.extend_from_slice(&command.to_le_bytes());
    bytes.extend_from_slice(&[0, 1, 0, 0, 0, 1]); // Call, READ, read activity.
    name(&mut bytes, label);
    bytes.extend_from_slice(input);
    bytes.extend_from_slice(output);
    bytes.push(0); // Unit progress; no returned access while UNKNOWN.
    bytes
}

fn interface(methods: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![0x71; 16];
    bytes.extend_from_slice(
        &u32::try_from(methods.len()).unwrap().to_le_bytes(),
    );
    for method in methods {
        bytes.extend_from_slice(method);
    }
    bytes
}

fn section_type() -> Vec<u8> {
    let mut bytes = vec![9, 3, 0]; // Record of name, offset and size.
    name(&mut bytes, "name");
    bytes.extend_from_slice(&[8, 0, 1, 0, 0]); // UTF-8, maximum 256 bytes.
    name(&mut bytes, "offset");
    bytes.push(4); // U64.
    name(&mut bytes, "size");
    bytes.push(4);
    bytes
}

fn reference(api: [u8; 16], rights: u32) -> Vec<u8> {
    let mut bytes = vec![12];
    bytes.extend_from_slice(&api);
    bytes.extend_from_slice(&rights.to_le_bytes());
    bytes
}

fn with_progress(mut method: Vec<u8>, progress: &[u8]) -> Vec<u8> {
    method.pop(); // Replace the independently encoded Unit progress.
    method.extend_from_slice(progress);
    method
}

fn factory(created: &[u8]) -> Vec<u8> {
    let mut bytes = method(1, "parse", &[0], &[0]);
    bytes[5] = 2; // Factory dispatch.
    bytes.extend_from_slice(created);
    bytes
}

#[test]
fn independent_control_declaration_retains_meaning() {
    let bytes = interface(&[
        method(1, "section_count", &[0], &[3]),
        method(2, "section", &[3], &section_type()),
    ]);
    let api = Declaration::decode(&bytes).unwrap();
    let section = api.methods().iter().find(|m| m.name() == "section").unwrap();
    assert_eq!(section.operation(), Operation::Ioctl(2));
    assert_eq!(section.dispatch(), Dispatch::Call);
    assert_eq!(section.access(), 1);
    assert_eq!(section.activity(), 1);
    assert_eq!(section.input(), &ValueType::decode(&[3]).unwrap());
    assert_eq!(section.output(), &ValueType::decode(&section_type()).unwrap());
    assert_eq!(Declaration::decode(api.bytes()).unwrap(), api);
}

#[test]
fn identity_and_complete_schema_both_participate_in_equality() {
    let mut bytes = interface(&[method(1, "count", &[0], &[3])]);
    let original = Declaration::decode(&bytes).unwrap();
    bytes[0] ^= 1;
    assert_ne!(Declaration::decode(&bytes).unwrap(), original);
    let changed = interface(&[method(1, "count", &[0], &[4])]);
    let changed = Declaration::decode(&changed).unwrap();
    assert_eq!(changed.api(), original.api());
    assert_ne!(changed, original); // Same key, different result meaning.
}

#[test]
fn unknown_progress_is_a_fixed_reference_free_value() {
    let mut progress = vec![9, 2, 0];
    name(&mut progress, "subject");
    progress.push(13); // Copied Identity is permitted.
    name(&mut progress, "accepted");
    progress.push(4); // U64 effect evidence.
    let entry = method(1, "publish", &[0], &[0]);
    let api = Declaration::decode(&interface(&[with_progress(
        entry.clone(),
        &progress,
    )]))
    .unwrap();
    assert_eq!(api.methods()[0].progress().bytes(), progress);
    assert!(api.methods()[0].created().is_none());
    assert_ne!(Declaration::decode(&interface(&[entry])).unwrap(), api);
}

#[test]
fn progress_cannot_hide_access_in_aggregate_types() {
    let access = reference([0; 16], 0);
    let mut record = vec![9, 1, 0];
    name(&mut record, "owned");
    record.extend_from_slice(&access);
    let mut sequence = vec![10, 0, 0, 0, 0]; // Even with no actual items.
    sequence.extend_from_slice(&record);
    let mut array = vec![14, 0, 0, 0, 0];
    array.extend_from_slice(&record);
    let mut choice = vec![11, 2, 0];
    choice.extend_from_slice(&0_u32.to_le_bytes());
    name(&mut choice, "empty");
    choice.push(0);
    choice.extend_from_slice(&1_u32.to_le_bytes());
    name(&mut choice, "owned");
    choice.extend_from_slice(&access);
    for progress in [access, record, sequence, array, choice] {
        let entry = with_progress(method(1, "run", &[0], &[0]), &progress);
        assert_eq!(
            Declaration::decode(&interface(&[entry])),
            Err(Error::INVALID)
        );
    }
}

#[test]
fn factory_primary_access_is_qualified_separately_from_auxiliary_output() {
    let tree = reference([0x53; 16], 1);
    let api = Declaration::decode(&interface(&[factory(&tree)])).unwrap();
    let parse = &api.methods()[0];
    assert_eq!(parse.dispatch(), Dispatch::Factory);
    assert_eq!(parse.created().unwrap().bytes(), tree);
    assert_eq!(parse.output().bytes(), &[0]);
    assert_eq!(parse.progress().bytes(), &[0]);
    let unopened = reference([0; 16], 0);
    assert!(Declaration::decode(&interface(&[factory(&unopened)])).is_ok());
    let changed = reference([0x53; 16], 3);
    assert_ne!(
        Declaration::decode(&interface(&[factory(&changed)])).unwrap(),
        api
    );
    assert_eq!(
        Declaration::decode(&interface(&[factory(&[13])])),
        Err(Error::INVALID)
    ); // Identity cannot own the created Resource.
    assert_eq!(
        Declaration::decode(&interface(&[factory(&[])])),
        Err(Error::INVALID)
    ); // No implied AnyResource or default rights.
}

#[test]
fn call_and_edit_cannot_declare_a_primary_creation() {
    for dispatch in [0, 1] {
        let mut entry = method(1, "run", &[0], &[0]);
        entry[5] = dispatch;
        entry.extend_from_slice(&reference([0; 16], 1));
        assert_eq!(
            Declaration::decode(&interface(&[entry])),
            Err(Error::INVALID)
        );
    }
}

#[test]
fn retired_record_without_progress_is_rejected() {
    let mut entry = method(1, "run", &[0], &[0]);
    entry.pop();
    assert_eq!(Declaration::decode(&interface(&[entry])), Err(Error::INVALID));
}

#[test]
fn truncation_and_trailing_bytes_cannot_create_a_binding() {
    let bytes = interface(&[method(1, "section", &[3], &section_type())]);
    for length in 0..bytes.len() {
        assert!(Declaration::decode(&bytes[..length]).is_err());
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(Declaration::decode(&trailing), Err(Error::INVALID));
}

#[test]
fn duplicate_selectors_names_and_noncanonical_order_reject() {
    let first = method(1, "first", &[0], &[0]);
    let second = method(2, "second", &[0], &[0]);
    let same_name = method(2, "first", &[0], &[0]);
    assert!(
        Declaration::decode(&interface(&[first.clone(), first.clone()]))
            .is_err()
    );
    assert!(
        Declaration::decode(&interface(&[first.clone(), same_name])).is_err()
    );
    assert!(Declaration::decode(&interface(&[second, first])).is_err());
}

#[test]
fn exec_requires_zero_command_and_exec_rights() {
    let mut entry = method(0, "run", &[0], &[0]);
    entry[0] = 1; // Exec.
    assert!(Declaration::decode(&interface(&[entry.clone()])).is_err());
    entry[6] = 4; // EXEC subject access.
    let api = Declaration::decode(&interface(&[entry.clone()])).unwrap();
    assert_eq!(api.methods()[0].operation(), Operation::Exec);
    entry[1] = 1;
    assert!(Declaration::decode(&interface(&[entry])).is_err());
    assert!(
        Declaration::decode(&interface(&[method(0, "bad", &[0], &[0])]))
            .is_err()
    );
}

#[test]
fn unknown_dispatch_access_and_activity_reject() {
    for offset in [0, 5, 6, 10] {
        let mut entry = method(1, "read", &[0], &[0]);
        entry[offset] = 255;
        assert!(Declaration::decode(&interface(&[entry])).is_err());
    }
}

#[test]
fn field_and_variant_names_and_tags_are_checked() {
    let mut duplicate = vec![9, 2, 0];
    name(&mut duplicate, "field");
    duplicate.push(0);
    name(&mut duplicate, "field");
    duplicate.push(3);
    assert!(ValueType::decode(&duplicate).is_err());
    let mut choice = vec![11, 2, 0];
    choice.extend_from_slice(&1_u32.to_le_bytes());
    name(&mut choice, "first");
    choice.push(0);
    choice.extend_from_slice(&1_u32.to_le_bytes());
    name(&mut choice, "second");
    choice.push(3);
    assert!(ValueType::decode(&choice).is_err());
    assert!(ValueType::decode(&[9, 1, 0, 0, 0, 3]).is_err());
    assert!(ValueType::decode(&[9, 1, 0, 1, 0, 255, 3]).is_err());
}

#[test]
fn reference_rights_allow_empty_but_reject_unknown_bits() {
    let mut reference = vec![12];
    reference.extend_from_slice(&[0; 16]); // AnyResource.
    reference.extend_from_slice(&1_u32.to_le_bytes());
    assert!(ValueType::decode(&reference).is_ok());
    reference[17] = 0;
    assert!(ValueType::decode(&reference).is_ok());
    reference[17] = 8;
    assert!(ValueType::decode(&reference).is_err());
}

#[test]
fn nesting_and_descriptor_bounds_are_enforced() {
    let mut nested = vec![0];
    for _ in 1..16 {
        let mut next = vec![14, 1, 0, 0, 0];
        next.extend_from_slice(&nested);
        nested = next;
    }
    assert!(ValueType::decode(&nested).is_ok());
    let mut excessive = vec![10, 1, 0, 0, 0];
    excessive.extend_from_slice(&nested);
    assert_eq!(ValueType::decode(&excessive), Err(Error::PRESSURE));
    assert_eq!(ValueType::decode(&vec![0; 65_537]), Err(Error::PRESSURE));
    assert_eq!(ValueType::decode(&[9, 1, 1]), Err(Error::PRESSURE));
}
