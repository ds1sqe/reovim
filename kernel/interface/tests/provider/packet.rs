use reovim_kernel_interface::{
    Error, Value,
    declaration::{ValueLimits, ValueType},
    provider::{Oversize, Receive},
};

#[test]
fn receive_envelope_preserves_packet_depth_and_counts_all_value_nodes() {
    // The fixed I/O envelope does not consume the packet's grammar depth.
    const PACKET_DEPTH: usize = 16;
    let mut descriptor = Vec::new();
    for _ in 0..PACKET_DEPTH - 1 {
        descriptor.extend_from_slice(&[9, 1, 0, 1, 0, b'v']);
    }
    descriptor.push(0);
    let packet = ValueType::decode(&descriptor).unwrap();
    let value = Value::copy(&[0; 9], &[]).unwrap();
    let request = Receive { maximum: 0, oversize: Oversize::Reject };
    let limits =
        ValueLimits { bytes: 9, elements: PACKET_DEPTH + 5, references: 0 };
    assert_eq!(
        request.validate(&packet, &value, limits, |_, _, _| unreachable!()),
        Ok(())
    );
    let limited = ValueLimits { elements: PACKET_DEPTH + 4, ..limits };
    assert_eq!(
        request.validate(&packet, &value, limited, |_, _, _| unreachable!()),
        Err(Error::PRESSURE)
    );
    // Prefix parsing is private to envelopes; an ordinary value still rejects
    // trailing bytes instead of silently accepting the same packet prefix.
    assert_eq!(
        packet.validate(&[0], 0, limits, |_, _, _| unreachable!()),
        Err(Error::INVALID)
    );
}
