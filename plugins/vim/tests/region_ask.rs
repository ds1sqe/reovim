//! A Plugin can pass the existing complete open record through Region.
mod common;

use reovim_client::dev::Clip;
use reovim_client_api::{NONE, open};
use reovim_client_region::Region;
use reovim_plugin_vim::Editor;
use reovim_rig::{Rig, lines, opened, status};
use reovim_sdk::cell;
use reovim_sdk::walk::ROOT;

#[test]
fn a_region_delivers_every_field_of_the_declared_ask() {
    let rig = Rig::new("region-full-ask");
    rig.file("a", b"one\ntwo\nthree\n");
    let host = rig.host();
    let editor = Editor::new();
    let mut tui = opened(
        &host,
        common::one(editor.clone()),
        Clip::none(),
        "",
        Some("a"),
        None,
    )
    .unwrap();
    tui.frame(80, 24).unwrap();
    let ctx = host.own();
    let region = Region::of(&ctx).unwrap();
    let target = cell::resolve(&ctx, ROOT, "/module/fs/root/a").unwrap();
    region
        .asks(
            &ctx,
            open::Ask {
                target,
                plugin: String::from("vim"),
                name: String::from("kept"),
                line: 2,
                locator: String::new(),
            },
        )
        .unwrap();
    tui.tick().unwrap();
    let ask = region.ask(&ctx).unwrap();
    assert_eq!(ask.target, target);
    assert_eq!(ask.plugin, "vim");
    assert_eq!(ask.name, "kept");
    assert_eq!(ask.line, 2);
    assert_eq!(editor.engine().position(), (1, 0));
    region
        .asks(
            &ctx,
            open::Ask {
                target: NONE,
                plugin: String::new(),
                name: String::from("sub/new"),
                line: 0,
                locator: String::new(),
            },
        )
        .unwrap();
    tui.tick().unwrap();
    let ask = region.ask(&ctx).unwrap();
    assert_eq!(ask.target, NONE);
    assert_eq!(ask.name, "sub/new");
    assert_eq!(lines(&mut tui), [""]);
    assert!(status(&mut tui).contains("new"));
    assert!(!rig.dir().join("sub/new").exists());
}
