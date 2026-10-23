//! Committed host declarations track the public APIs.

#[test]
fn committed_renderings_match() {
    assert_eq!(
        include_str!("../search.search.fipe"),
        reovim_search_api::search::text("search"),
    );
}
