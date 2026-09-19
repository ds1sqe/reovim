//! `drivers!`: the Module a set of declared apis under one `bin` is.
//! Two Modules spell that shape and differ in a name and a list, so it
//! is written once here and they are written as the name and the list.

/// A Module of Drivers: one Driver, one name per declared api under the
/// nearest `bin` above its home, and an output cell per run.
///
/// Each api is a module of [`declare!`](crate::declare) in scope at the
/// call, named as the name it is registered under, in the order they
/// are registered in. It generates `boot`, the type with its `Default`,
/// the `cell` and `following` a run answers with, `Module::start` and
/// the whole `Driver`; beside each api is written that api's `Api`.
///
/// ```ignore
/// pub mod bins;
/// use bins::{cat, grep, ls, stat, tree};
///
/// reovim_sdk::drivers!(Coreutils: stat, ls, tree, cat, grep);
/// ```
#[macro_export]
macro_rules! drivers {
    ($module:ident : $( $api:ident ),+ $(,)?) => {
        // Every name the shape below spells, as a glob: what a Module
        // imports for its own code shadows it and never collides.
        use $crate::__interface::*;

        /// The one Driver, which is `Booted::drivers[0]`: every name
        /// this Module registers is served by it, and so is every
        /// output cell.
        const DRIVER: DriverId = DriverId(0);

        /// The rd each name is served at: a Container, so a run's
        /// output cell goes under it, and READ, so a caller may hold
        /// the right `run` asks for.
        const NODE: Flags = Flags::READ.union(Flags::CONTAINER);

        /// What is registered under the `bin`, in the order of `decls`
        /// and `hashes`.
        const NAMES: &[&str] = &[$( stringify!($api) ),+];

        /// The Module and its Driver in one; pass it twice at `load`.
        pub struct $module {
            /// One per name of `NAMES`, reported at that name's rd.
            decls: [Decl; NAMES.len()],
            hashes: [Hash; NAMES.len()],
            cells: $crate::cell::Cells,
        }

        /// The Module built and ready for `load`.
        #[must_use]
        pub fn boot(_root: &str) -> Booted {
            let it = Rc::new($module::default());
            Booted { module: it.clone(), drivers: vec![it] }
        }

        /// The Module, before it is loaded.
        impl Default for $module {
            fn default() -> Self {
                let decls = [$( $api::decl() ),+];
                let hashes = decls.each_ref().map(Decl::hash);
                let cells = $crate::cell::Cells::default();
                Self { decls, hashes, cells }
            }
        }

        impl $module {
            /// The output cell a run answers with: `bytes` under `rd`,
            /// the rd the run was called on, which is the name's own.
            fn cell(
                &self,
                ctx: &dyn Calls,
                rd: Rd,
                bytes: Vec<u8>,
            ) -> Result<Rd, Error> {
                self.cells.open(ctx, DRIVER, rd, bytes)
            }

            /// The output cell of a run that goes on past what it found
            /// now: `follow` is what a read at the cell's end asks.
            fn following(
                &self,
                ctx: &dyn Calls,
                rd: Rd,
                bytes: Vec<u8>,
                follow: Box<dyn $crate::cell::Follow>,
            ) -> Result<Rd, Error> {
                self.cells.follow(ctx, DRIVER, rd, bytes, follow)
            }
        }

        impl Module for $module {
            fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
                let bin = $crate::cell::bin(ctx, home)?;
                for (name, decl) in NAMES.iter().zip(&self.decls) {
                    let rd = ctx.register(DRIVER, bin, Some(name))?;
                    ctx.report(rd, NODE, ::core::slice::from_ref(decl))?;
                }
                Ok(())
            }
        }

        impl Driver for $module {
            fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
                Reply::ok(rd.0)
            }

            fn read(
                &self,
                ctx: &dyn Calls,
                h: u64,
                at: u64,
                len: usize,
            ) -> Reply<Vec<u8>> {
                self.cells.read(ctx, Rd(h), at, len).into()
            }

            /// The api the Kernel checked the rd offers says which run
            /// this is; the rd called on is the one the output cell
            /// goes under.
            fn call(
                &self,
                ctx: &dyn Calls,
                h: u64,
                api: Hash,
                number: u16,
                input: &[u8],
            ) -> Reply<Vec<u8>> {
                let [$( $api ),+] = self.hashes;
                let rd = Rd(h);
                let ran = match api {
                    $( a if a == $api => {
                        $api::dispatch(self, ctx, rd, number, input)
                    } )+
                    _ => Err(Error::Unsupported),
                };
                ran.into()
            }

            fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
                self.cells.close(ctx, Rd(h)).into()
            }
        }
    };
}
