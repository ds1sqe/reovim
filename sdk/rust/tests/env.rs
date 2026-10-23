//! The environment walk over the Client and cli's real providers.

use std::rc::Rc;

use reovim_client::{Clip, Host, dev};
use reovim_kernel_interface::{Calls, DriverId, Error, Flags, Rd};
use reovim_rig::Rig;
use reovim_sdk::env::{self, Level};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, log, ops};

fn client(rig: &Rig) -> Rc<Host> {
    let host = rig.host();
    dev::boot(&host, Box::new(|_| None), Clip::none(), "", None, None, "")
        .unwrap();
    let cli = reovim_plugin_cli::boot();
    host.load("cli", cli.module, cli.drivers).unwrap();
    host
}

fn surface(ctx: &dyn Calls) -> Rd {
    cell::resolve(ctx, ROOT, "/self/cli").unwrap()
}

fn last_log(ctx: &dyn Calls) -> String {
    let rd = cell::resolve(ctx, ROOT, "/sys/log").unwrap();
    log::parse(&ops::bytes(ctx, rd).unwrap()).last().unwrap().text()
}

#[test]
fn client_values_are_inherited_and_surface_values_shadow_them() {
    let rig = Rig::new("env-inherit");
    let host = client(&rig);
    let ctx = host.own();
    let from = surface(&ctx);
    assert_eq!(env::get(&ctx, from, "PATH"), Ok(None));

    env::set(&ctx, from, Level::Client, "PATH", "/bin").unwrap();
    assert_eq!(last_log(&ctx), "env: client PATH=/bin");
    assert_eq!(env::get(&ctx, from, "PATH"), Ok(Some("/bin".into())));
    env::set(&ctx, from, Level::Surface, "PATH", "/local").unwrap();
    assert_eq!(last_log(&ctx), "env: surface PATH=/local");
    assert_eq!(env::get(&ctx, from, "PATH"), Ok(Some("/local".into())));

    env::set(&ctx, from, Level::Surface, "PATH", "").unwrap();
    assert_eq!(last_log(&ctx), "env: surface PATH=");
    assert_eq!(env::get(&ctx, from, "PATH"), Ok(Some(String::new())));
    let self_rd = cell::resolve(&ctx, ROOT, "/self").unwrap();
    assert_eq!(env::get(&ctx, self_rd, "PATH"), Ok(Some("/bin".into())));
    assert_eq!(env::get(&ctx, from, "missing"), Ok(None));
}

#[test]
fn missing_levels_are_unsupported_and_unrelated_apis_are_skipped() {
    let rig = Rig::new("env-levels");
    let host = client(&rig);
    let ctx = host.own();
    let from = surface(&ctx);
    assert_eq!(
        env::set(&ctx, from, Level::Session, "key", "value"),
        Err(Error::Unsupported)
    );
    let self_rd = cell::resolve(&ctx, ROOT, "/self").unwrap();
    let inner = ctx.register(DriverId(0), self_rd, Some("inner")).unwrap();
    ctx.report(inner, Flags::READ.union(Flags::CONTAINER), &[]).unwrap();
    assert_eq!(
        env::set(&ctx, inner, Level::Surface, "key", "value"),
        Err(Error::Unsupported)
    );
    let unrelated = ctx.register(DriverId(0), inner, Some("env")).unwrap();
    ctx.report(unrelated, Flags::READ, &[]).unwrap();
    env::set(&ctx, from, Level::Client, "key", "outer").unwrap();
    assert_eq!(env::get(&ctx, inner, "key"), Ok(Some("outer".into())));
}

#[test]
fn wire_bounds_reject_oversized_values_without_changing_the_entry() {
    let rig = Rig::new("env-bounds");
    let host = client(&rig);
    let ctx = host.own();
    let from = surface(&ctx);
    env::set(&ctx, from, Level::Client, "key", "kept").unwrap();
    assert_eq!(
        env::set(&ctx, from, Level::Client, "key", &"x".repeat(4097)),
        Err(Error::Invalid)
    );
    assert_eq!(env::get(&ctx, from, "key"), Ok(Some("kept".into())));
    assert_eq!(last_log(&ctx), "env: client key=kept");
    assert_eq!(env::get(&ctx, from, &"x".repeat(257)), Err(Error::Invalid));
}

#[test]
fn environment_values_belong_to_their_client_and_surface_lifetime() {
    let rig = Rig::new("env-lifetime");
    let host = client(&rig);
    let ctx = host.own();
    let from = surface(&ctx);
    env::set(&ctx, from, Level::Client, "key", "client").unwrap();
    env::set(&ctx, from, Level::Surface, "key", "surface").unwrap();
    for path in ["/self/env", "/self/cli/env"] {
        let rd = cell::resolve(&ctx, ROOT, path).unwrap();
        assert!(!ops::info(&ctx, rd).unwrap().flags.contains(Flags::CONTAINER));
        assert_eq!(ops::children(&ctx, rd), Ok(Vec::new()));
        assert_eq!(ops::bytes(&ctx, rd), Err(Error::Unsupported));
    }
    let other = client(&rig);
    assert_eq!(env::get(&other.own(), surface(&other.own()), "key"), Ok(None));
    host.unload("cli").unwrap();
    let cli = reovim_plugin_cli::boot();
    host.load("cli", cli.module, cli.drivers).unwrap();
    assert_eq!(env::get(&ctx, surface(&ctx), "key"), Ok(Some("client".into())));
}
