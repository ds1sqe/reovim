mod intent;
mod session;
mod update;

pub const fn all() -> bool {
    session::all() && intent::all() && update::all()
}
