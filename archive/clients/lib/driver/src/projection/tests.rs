use {
    super::*,
    reovim_subsys_coordination::{DomainId, ProjectionTag},
};

#[test]
fn domain_projection_new() {
    let tag = ProjectionTag::new("text.mode");
    let proj = DomainProjection::new(tag.clone(), DomainId(1), b"normal".to_vec(), "NORMAL".into());

    assert_eq!(proj.tag(), &tag);
    assert_eq!(proj.domain_id, DomainId(1));
    assert_eq!(proj.content, b"normal");
    assert_eq!(proj.display, "NORMAL");
    assert!(!proj.transient);
    assert_eq!(proj.version, 0);
    assert!(!proj.is_windowed());
}

#[test]
fn domain_projection_windowed() {
    let mut proj = DomainProjection::new(
        ProjectionTag::new("text.cursor"),
        DomainId(1),
        vec![],
        String::new(),
    );
    assert!(!proj.is_windowed());

    proj.window_id = Some(42);
    assert!(proj.is_windowed());
}

#[cfg(feature = "proto")]
mod proto_tests {
    use {
        reovim_protocol::v3::{DomainDatum, ProjectionUpdatedPayload},
        reovim_subsys_coordination::{DomainId, ProjectionTag},
    };

    use super::super::{TranspileError, from_proto, transpiler::try_from_proto};

    fn make_payload(
        tag: &str,
        domain_id: u32,
        datum: Option<DomainDatum>,
    ) -> ProjectionUpdatedPayload {
        ProjectionUpdatedPayload {
            tag: tag.to_string(),
            domain_id,
            window_id: None,
            datum,
            transient: false,
            version: 5,
            client_id: 99,
        }
    }

    #[test]
    fn from_proto_happy_path() {
        let datum = DomainDatum {
            content: b"insert".to_vec(),
            display: Some("INSERT".to_string()),
        };
        let payload = make_payload("text.mode", 1, Some(datum));

        let proj = from_proto(payload);

        assert_eq!(proj.tag(), &ProjectionTag::new("text.mode"));
        assert_eq!(proj.domain_id, DomainId(1));
        assert_eq!(proj.content, b"insert");
        assert_eq!(proj.display, "INSERT");
        assert!(!proj.transient);
        assert_eq!(proj.version, 5);
        assert_eq!(proj.client_id, 99);
        assert!(!proj.is_windowed());
    }

    #[test]
    fn from_proto_with_window_id() {
        let datum = DomainDatum {
            content: vec![],
            display: None,
        };
        let mut payload = make_payload("text.cursor", 1, Some(datum));
        payload.window_id = Some(7);

        let proj = from_proto(payload);
        assert_eq!(proj.window_id, Some(7));
        assert!(proj.is_windowed());
    }

    #[test]
    fn from_proto_transient() {
        let datum = DomainDatum {
            content: vec![],
            display: None,
        };
        let mut payload = make_payload("text.flash", 1, Some(datum));
        payload.transient = true;
        payload.version = 0;

        let proj = from_proto(payload);
        assert!(proj.transient);
        assert_eq!(proj.version, 0);
    }

    #[test]
    fn try_from_proto_missing_datum() {
        let payload = make_payload("text.mode", 1, None);
        let result = try_from_proto(payload);
        assert_eq!(result.unwrap_err(), TranspileError::MissingDatum);
    }

    #[test]
    fn try_from_proto_window_id_oversized_u64_yields_none_on_narrow_usize() {
        // Regression guard for the `u64 → usize` conversion in `try_from_proto`:
        // a window_id that does not fit in `usize` (only reachable on 32-bit
        // targets) maps to `None` rather than silently truncating. On 64-bit
        // targets any `u64` fits in `usize`, so the expected value there is
        // `Some(id as usize)`.
        let datum = DomainDatum {
            content: vec![],
            display: None,
        };
        let mut payload = make_payload("text.cursor", 1, Some(datum));
        let oversized: u64 = u64::MAX;
        payload.window_id = Some(oversized);

        let proj = try_from_proto(payload).expect("transpile");
        let expected = usize::try_from(oversized).ok();
        assert_eq!(proj.window_id, expected);
    }

    #[test]
    #[should_panic(expected = "server contract violation")]
    fn from_proto_panics_on_missing_datum() {
        let payload = make_payload("text.mode", 1, None);
        let _ = from_proto(payload);
    }

    #[test]
    fn newtype_reconstruction_preserves_tag_string() {
        let datum = DomainDatum {
            content: vec![],
            display: None,
        };
        let payload = make_payload("3d.mesh.transform", 42, Some(datum));
        let proj = from_proto(payload);

        assert_eq!(proj.tag().as_str(), "3d.mesh.transform");
        assert_eq!(proj.domain_id, DomainId(42));
    }
}
