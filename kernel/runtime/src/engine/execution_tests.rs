use reovim_kernel_interface::{Error, Outcome, ResourceId};

#[test]
fn typed_provider_error_metadata_obeys_the_native_boundary() {
    assert_eq!(
        super::check_error(&Outcome::Failed(Error::new(0))),
        Err(Error::PROVIDER)
    );
    let invalid = Error::with_id(91, ResourceId { epoch: [1; 16], serial: 0 });
    assert_eq!(
        super::check_error(&Outcome::Unknown {
            error: invalid,
            progress: None,
        }),
        Err(Error::PROVIDER)
    );
    let valid =
        Error::with_id(u32::MAX, ResourceId { epoch: [1; 16], serial: 17 });
    assert_eq!(
        super::check_error(&Outcome::Unknown { error: valid, progress: None }),
        Ok(())
    );
}
