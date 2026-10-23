//! Committed host declarations track the public APIs.

#[test]
fn committed_renderings_match() {
    assert_eq!(
        include_str!("../client.keyboard.fipe"),
        reovim_client_api::keyboard::text("client"),
    );
    assert_eq!(
        include_str!("../client.window.fipe"),
        reovim_client_api::window::text("client"),
    );
    assert_eq!(
        include_str!("../client.display.fipe"),
        reovim_client_api::display::text("client"),
    );
    assert_eq!(
        include_str!("../client.looks.fipe"),
        reovim_client_api::looks::text("client"),
    );
    assert_eq!(
        include_str!("../client.open.fipe"),
        reovim_client_api::open::text("client"),
    );
    assert_eq!(
        include_str!("../client.buffers.fipe"),
        reovim_client_api::buffers::text("client"),
    );
    assert_eq!(
        include_str!("../client.draft.fipe"),
        reovim_client_api::draft::text("client"),
    );
    assert_eq!(
        include_str!("../client.resolver.fipe"),
        reovim_client_api::resolver::text("client"),
    );
}
