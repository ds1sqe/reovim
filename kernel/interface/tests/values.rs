use reovim_kernel_interface::{
    Error,
    declaration::{ValueLimits, ValueType},
};

const LIMITS: ValueLimits =
    ValueLimits { bytes: 512, elements: 32, references: 8 };

fn name(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&u16::try_from(value.len()).unwrap().to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

fn reference(api: Option<[u8; 16]>, rights: u32) -> Vec<u8> {
    let mut bytes = vec![12];
    bytes.extend_from_slice(&api.unwrap_or([0; 16]));
    bytes.extend_from_slice(&rights.to_le_bytes());
    bytes
}

fn record(fields: &[(&str, &[u8])]) -> ValueType {
    let mut bytes = vec![9];
    bytes
        .extend_from_slice(&u16::try_from(fields.len()).unwrap().to_le_bytes());
    for (label, field) in fields {
        name(&mut bytes, label);
        bytes.extend_from_slice(field);
    }
    ValueType::decode(&bytes).unwrap()
}

fn sized(tag: u8, bound: u32, element: &[u8]) -> Vec<u8> {
    let mut bytes = vec![tag];
    bytes.extend_from_slice(&bound.to_le_bytes());
    bytes.extend_from_slice(element);
    bytes
}

fn payload(bytes: &[u8]) -> Vec<u8> {
    let mut data = u32::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
    data.extend_from_slice(bytes);
    data
}

fn validate(ty: &ValueType, data: &[u8]) -> Result<(), Error> {
    ty.validate(data, 0, LIMITS, |_, _, _| unreachable!())
}

#[test]
fn ascii_identifiers_do_not_restrict_value_contents() {
    let text = record(&[("content", &sized(8, 32, &[]))]);
    assert_eq!(validate(&text, &payload("内容".as_bytes())), Ok(()));
    let bytes = record(&[("content", &sized(7, 32, &[]))]);
    assert_eq!(validate(&bytes, &payload(b"\xff\0\xfe")), Ok(()));
}

#[test]
fn reference_reservations_follow_shapes_and_admission_bounds() {
    let reference = reference(None, 1);
    let sequence = sized(10, 3, &reference);
    let repeated = ValueType::decode(&sequence).unwrap();
    assert_eq!(repeated.reference_bound(8), 3);
    assert_eq!(repeated.reference_bound(2), 2);
    let pair = record(&[("first", &reference), ("rest", &sequence)]);
    assert_eq!(pair.reference_bound(8), 4);
    let huge = sized(14, u32::MAX, &sized(10, u32::MAX, &reference));
    assert_eq!(ValueType::decode(&huge).unwrap().reference_bound(8), 8);
    let bytes = ValueType::decode(&sized(7, 12, &[])).unwrap();
    assert_eq!(bytes.reference_bound(8), 0);
    let mut choice = vec![11, 2, 0];
    choice.extend_from_slice(&0_u32.to_le_bytes());
    name(&mut choice, "one");
    choice.extend_from_slice(&reference);
    choice.extend_from_slice(&1_u32.to_le_bytes());
    name(&mut choice, "many");
    choice.extend_from_slice(&sequence);
    assert_eq!(ValueType::decode(&choice).unwrap().reference_bound(8), 3);
}

#[test]
fn zero_right_reference_still_checks_its_access_owner() {
    let ty = ValueType::decode(&reference(None, 0)).unwrap();
    let slot = 0_u32.to_le_bytes();
    let mut observed = Vec::new();
    assert_eq!(
        ty.validate(&slot, 1, LIMITS, |index, api, rights| {
            observed.push((index, api, rights));
            Ok(())
        }),
        Ok(())
    );
    assert_eq!(observed, vec![(0, None, 0)]);
    assert_eq!(
        ty.validate(&slot, 1, LIMITS, |_, _, _| Err(Error::CLOSED)),
        Err(Error::CLOSED)
    );
}

#[test]
fn repeated_slot_checks_each_api_and_rights_requirement() {
    let api = [0x71; 16];
    let opened = reference(None, 0);
    let read = reference(Some(api), 1);
    let ty = record(&[("opened", &opened), ("read", &read)]);
    let data = [0; 8]; // Both fields use the same access slot.
    let mut observed = Vec::new();
    assert_eq!(
        ty.validate(&data, 1, LIMITS, |index, expected, rights| {
            observed.push((index, expected, rights));
            Ok(())
        }),
        Ok(())
    );
    assert_eq!(observed, vec![(0, None, 0), (0, Some(api), 1)]);
    assert_eq!(
        ty.validate(&data, 1, LIMITS, |_, _, rights| {
            if rights == 0 { Ok(()) } else { Err(Error::DENIED) }
        }),
        Err(Error::DENIED)
    );
}

#[test]
fn invalid_or_unused_slots_cannot_supply_hidden_access() {
    let ty = ValueType::decode(&reference(None, 0)).unwrap();
    assert_eq!(
        ty.validate(&1_u32.to_le_bytes(), 1, LIMITS, |_, _, _| {
            unreachable!()
        }),
        Err(Error::INVALID)
    );
    assert_eq!(validate(&ty, &0_u32.to_le_bytes()), Err(Error::INVALID));
    let unit = ValueType::decode(&[0]).unwrap();
    assert_eq!(
        unit.validate(&[], 1, LIMITS, |_, _, _| unreachable!()),
        Err(Error::INVALID)
    );
}

#[test]
fn choice_checks_only_the_selected_reference_value() {
    let mut schema = vec![11, 2, 0];
    schema.extend_from_slice(&0_u32.to_le_bytes());
    name(&mut schema, "None");
    schema.push(0);
    schema.extend_from_slice(&1_u32.to_le_bytes());
    name(&mut schema, "Some");
    schema.extend_from_slice(&reference(None, 1));
    let ty = ValueType::decode(&schema).unwrap();
    assert_eq!(validate(&ty, &0_u32.to_le_bytes()), Ok(()));
    let mut some = 1_u32.to_le_bytes().to_vec();
    some.extend_from_slice(&0_u32.to_le_bytes());
    assert_eq!(
        ty.validate(&some, 1, LIMITS, |_, _, _| Err(Error::DENIED)),
        Err(Error::DENIED)
    );
    assert_eq!(validate(&ty, &2_u32.to_le_bytes()), Err(Error::INVALID));
}

#[test]
fn nested_sequences_resume_at_the_next_record_field() {
    let pair = sized(14, 2, &[3]); // Array of two U32 values.
    let pairs = sized(10, 3, &pair);
    let ty = record(&[("pairs", &pairs), ("complete", &[1])]);
    let mut data = 2_u32.to_le_bytes().to_vec();
    for coordinate in [11_u32, 12, 21, 22] {
        data.extend_from_slice(&coordinate.to_le_bytes());
    }
    data.push(1); // Complete is true, after both pairs.
    assert_eq!(validate(&ty, &data), Ok(()));
    *data.last_mut().unwrap() = 2; // Invalid Bool after valid sequence data.
    assert_eq!(validate(&ty, &data), Err(Error::INVALID));
}

#[test]
fn empty_sequence_still_advances_past_its_element_schema() {
    let pairs = sized(10, 3, &sized(14, 2, &[3]));
    let ty = record(&[("pairs", &pairs), ("complete", &[1])]);
    let mut data = 0_u32.to_le_bytes().to_vec();
    data.push(1);
    assert_eq!(validate(&ty, &data), Ok(()));
    data.pop();
    assert_eq!(validate(&ty, &data), Err(Error::INVALID));
}

#[test]
fn utf8_validation_does_not_transcode_opaque_bytes() {
    let text = ValueType::decode(&sized(8, 4, &[])).unwrap();
    let bytes = ValueType::decode(&sized(7, 4, &[])).unwrap();
    let accented = payload("é".as_bytes());
    assert_eq!(validate(&text, &accented), Ok(()));
    assert_eq!(validate(&text, &payload(&[0xff])), Err(Error::INVALID));
    assert_eq!(validate(&bytes, &payload(&[0xff])), Ok(()));
    let short = ValueType::decode(&sized(8, 1, &[])).unwrap();
    assert_eq!(validate(&short, &accented), Err(Error::INVALID));
}

#[test]
fn identity_is_metadata_with_a_nonzero_serial() {
    let ty = ValueType::decode(&[13]).unwrap();
    let mut identity = vec![0x32; 16];
    identity.extend_from_slice(&42_u64.to_le_bytes());
    assert_eq!(validate(&ty, &identity), Ok(()));
    identity[16..].fill(0);
    assert_eq!(validate(&ty, &identity), Err(Error::INVALID));
}

#[test]
fn scalar_validation_rejects_truncation_and_trailing_data() {
    let boolean = ValueType::decode(&[1]).unwrap();
    assert_eq!(validate(&boolean, &[1]), Ok(()));
    assert_eq!(validate(&boolean, &[2]), Err(Error::INVALID));
    assert_eq!(validate(&boolean, &[1, 0]), Err(Error::INVALID));
    let integer = ValueType::decode(&[4]).unwrap();
    let selected = 0x1234_5678_9abc_def0_u64.to_le_bytes();
    assert_eq!(validate(&integer, &selected), Ok(()));
    assert_eq!(validate(&integer, &selected[..7]), Err(Error::INVALID));
    let float = ValueType::decode(&[6]).unwrap();
    assert_eq!(validate(&float, &f64::NAN.to_le_bytes()), Ok(()));
    // Finite geometry is a domain invariant, not a primitive F64 constraint.
}

#[test]
fn declared_bounds_and_runtime_pressure_have_distinct_failures() {
    let units = ValueType::decode(&sized(10, 100, &[0])).unwrap();
    assert_eq!(validate(&units, &101_u32.to_le_bytes()), Err(Error::INVALID));
    assert_eq!(validate(&units, &50_u32.to_le_bytes()), Err(Error::PRESSURE));
    let boolean = ValueType::decode(&[1]).unwrap();
    let no_bytes = ValueLimits { bytes: 0, ..LIMITS };
    assert_eq!(
        boolean.validate(&[1], 0, no_bytes, |_, _, _| unreachable!()),
        Err(Error::PRESSURE)
    );
    let reference = ValueType::decode(&reference(None, 0)).unwrap();
    let no_refs = ValueLimits { references: 0, ..LIMITS };
    assert_eq!(
        reference.validate(&[0; 4], 1, no_refs, |_, _, _| unreachable!()),
        Err(Error::PRESSURE)
    );
}

#[test]
fn reference_validation_preserves_unrecognized_machine_errors() {
    let ty = ValueType::decode(&reference(None, 0)).unwrap();
    let cause = Error::new(90_001);
    assert_eq!(
        ty.validate(&[0; 4], 1, LIMITS, |_, _, _| Err(cause)),
        Err(cause)
    );
}
