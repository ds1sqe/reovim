use reovim_bitflags::rv_bitflags;

rv_bitflags! {
    pub struct Access: u32 {
        const READ = 1;
        const WRITE = 2;
        const WATCH = 8;
        const EDIT = Self::READ.bits() | Self::WRITE.bits();
    }
}

#[test]
fn flags_preserve_grant_relations() {
    assert_eq!(Access::READ | Access::WRITE, Access::EDIT);
    assert!(Access::EDIT.contains(Access::READ));
    assert!(!Access::READ.contains(Access::EDIT));
    assert_eq!(Access::empty() | Access::READ, Access::READ);
    assert_eq!(Access::empty().bits(), 0);
}

#[test]
fn raw_bits_must_belong_to_the_declared_domain() {
    assert_eq!(Access::from_bits(Access::EDIT.bits()), Some(Access::EDIT));
    assert_eq!(Access::from_bits(0), Some(Access::empty()));
    // The declared bits need not run on: 8 belongs, 4 does not.
    assert_eq!(Access::from_bits(Access::WATCH.bits()), Some(Access::WATCH));
    assert_eq!(Access::from_bits(4), None);
    assert_eq!(Access::from_bits(Access::READ.bits() | 4), None);
}
