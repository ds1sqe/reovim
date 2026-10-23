//! Committed host declarations track the public APIs.

#[test]
fn committed_renderings_match() {
    assert_eq!(
        include_str!("../text.open.fipe"),
        reovim_text_api::open::text("text"),
    );
    assert_eq!(
        include_str!("../text.text.fipe"),
        reovim_text_api::text::text("text"),
    );
    assert_eq!(
        include_str!("../text.history.fipe"),
        reovim_text_api::history::text("text"),
    );
    assert_eq!(
        include_str!("../text.publish.fipe"),
        reovim_text_api::publish::text("text"),
    );
}
