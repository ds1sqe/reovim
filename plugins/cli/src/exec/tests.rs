use super::*;
use core::cell::RefCell;
use reovim_kernel_interface::{
    DriverId, Flags, Handle, Kind, Op, Out, Outcome as Finished, Rd, Rights,
    Token,
};

struct Tracked<'a> {
    ctx: &'a dyn Calls,
    open: RefCell<Vec<Handle>>,
}

impl Calls for Tracked<'_> {
    fn register(
        &self,
        driver: DriverId,
        parent: Rd,
        name: Option<&str>,
    ) -> Result<Rd, Error> {
        self.ctx.register(driver, parent, name)
    }
    fn name(&self, rd: Rd, name: Option<&str>) -> Result<(), Error> {
        self.ctx.name(rd, name)
    }
    fn report(&self, rd: Rd, flags: Flags, apis: &[Decl]) -> Result<(), Error> {
        self.ctx.report(rd, flags, apis)
    }
    fn unregister(&self, rd: Rd) -> Result<(), Error> {
        self.ctx.unregister(rd)
    }
    fn changed(&self, rd: Rd, kind: Kind) -> Result<(), Error> {
        self.ctx.changed(rd, kind)
    }
    fn done(&self, token: Token, result: Finished) -> Result<(), Error> {
        self.ctx.done(token, result)
    }
    fn open(&self, rd: Rd, rights: Rights) -> Result<Handle, Error> {
        let h = self.ctx.open(rd, rights)?;
        self.open.borrow_mut().push(h);
        Ok(h)
    }
    fn submit(&self, op: Op) -> reovim_kernel_interface::Reply<Out> {
        let closing = match &op {
            Op::Close { h } => Some(*h),
            _ => None,
        };
        let reply = self.ctx.submit(op);
        if let Some(h) = closing {
            assert!(matches!(
                reply,
                reovim_kernel_interface::Reply::Ready(Ok(_))
            ));
            self.open.borrow_mut().retain(|open| *open != h);
        }
        reply
    }
}

#[test]
fn declarations_load() {
    let mut declarations = Declarations::new();
    declarations
        .commands(include_str!("../../../../sdk/rust/commands.fipe"))
        .unwrap();
    for api in apis() {
        let name = format!("{}.{}", api.home, api.decl.api);
        declarations
            .module(&name, &api.text)
            .unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
    }
}

#[test]
fn done_host_fault_and_budget_close_every_handle() {
    let rig = reovim_rig::Rig::new("fipe-handles");
    let host = rig.host();
    let ctx = host.own();
    let tracked = Tracked { ctx: &ctx, open: RefCell::new(Vec::new()) };
    let cases = [
        (
            concat!(
                "use text.open\n",
                "text.open.new (text.open.at \"/module/text/open\")\n",
                "\"done\""
            ),
            "\"done\"",
            1000,
        ),
        (
            "use text.open\ntext.open.at \"/module/text/missing\"",
            "text.open.at: Invalid",
            1000,
        ),
        (
            concat!(
                "use text.open\n",
                "h = text.open.at \"/module/text/open\"\n",
                "loop: I64 -> I64 = n => loop (n + 1)\nloop 0"
            ),
            "budget is exhausted",
            1000,
        ),
    ];
    for (text, expected, budget) in cases {
        let mut executor = Executor::new().unwrap();
        let (_, output) =
            executor.run(&tracked, "/test.fipe", text, budget).unwrap();
        assert!(output.contains(expected), "{output}");
        assert!(tracked.open.borrow().is_empty(), "{output}");
        let (_, output) =
            executor.run(&tracked, "/next.fipe", "42", 1000).unwrap();
        assert_eq!(output, "42\n");
    }
}

#[test]
fn imports_resolve_beside_the_file_and_errors_use_that_source() {
    let rig = reovim_rig::Rig::new("fipe-import");
    rig.file("helper.fipe", b"pub greet: String -> String = name => name\n");
    let host = rig.host();
    let ctx = host.own();
    let mut executor = Executor::new().unwrap();
    let (_, output) = executor
        .run(
            &ctx,
            "/module/fs/root/main.fipe",
            "use helper\nhelper.greet \"neighbor\"",
            1000,
        )
        .unwrap();
    assert_eq!(output, "\"neighbor\"\n");
    rig.file("bad.fipe", b"pub bad: I64 = True\n");
    let (_, output) = executor
        .run(&ctx, "/module/fs/root/main.fipe", "use bad", 1000)
        .unwrap();
    assert!(output.contains("bad.fipe:1:"), "{output}");
    assert!(output.contains("expected I64, found Bool"), "{output}");
    let (_, output) = executor
        .run(
            &ctx,
            "/module/fs/root/main.fipe",
            "use helper\nhelper.hidden",
            1000,
        )
        .unwrap();
    assert!(output.contains("no such member"), "{output}");
    let diagnostic = Diagnostic::Compile {
        span: fipe::token::Span::default(),
        what: fipe::check::Evaluated::Demand,
        reason: fipe::value::Reason::Message("first\nsecond".into()),
    };
    assert_eq!(
        diagnostic_line(&ctx, "/compile.fipe", "", &diagnostic),
        "/compile.fipe:1:1: first\\nsecond\n"
    );
}

#[derive(Default)]
struct Counting {
    opens: core::cell::Cell<usize>,
    calls: core::cell::Cell<usize>,
    closes: core::cell::Cell<usize>,
}

impl reovim_kernel_interface::Module for Counting {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let rd = ctx.register(DriverId(0), home, Some("open"))?;
        ctx.report(
            rd,
            Flags::READ.union(Flags::WRITE),
            &[reovim_text_api::open::decl()],
        )
    }
}

impl reovim_kernel_interface::Driver for Counting {
    fn open(
        &self,
        _: &dyn Calls,
        rd: Rd,
        _: Rights,
    ) -> reovim_kernel_interface::Reply<u64> {
        self.opens.set(self.opens.get() + 1);
        reovim_kernel_interface::Reply::ok(rd.0)
    }

    fn call(
        &self,
        _: &dyn Calls,
        h: u64,
        _: reovim_kernel_interface::Hash,
        _: u16,
        _: &[u8],
    ) -> reovim_kernel_interface::Reply<Vec<u8>> {
        self.calls.set(self.calls.get() + 1);
        reovim_kernel_interface::Reply::ok(reovim_sdk::encode(&Rd(h)))
    }

    fn close(
        &self,
        _: &dyn Calls,
        _: u64,
    ) -> reovim_kernel_interface::Reply<()> {
        self.closes.set(self.closes.get() + 1);
        reovim_kernel_interface::Reply::ok(())
    }
}

#[test]
fn calls_reuse_the_handle_at_opened() {
    let rig = reovim_rig::Rig::new("fipe-handle-reuse");
    let host = rig.host();
    let driver = alloc::rc::Rc::new(Counting::default());
    host.load("counted", driver.clone(), alloc::vec![driver.clone()]).unwrap();
    let ctx = host.own();
    let mut executor = Executor::new().unwrap();
    let (_, output) = executor
        .run(
            &ctx,
            "/reuse.fipe",
            concat!(
                "use text.open\n",
                "h = text.open.at \"/self/counted/open\"\n",
                "text.open.new h\ntext.open.new h",
            ),
            1000,
        )
        .unwrap();
    assert_eq!(output, "0\n0\n");
    assert_eq!(driver.opens.get(), 1, "one at, no opens at calls");
    assert_eq!(driver.calls.get(), 2);
    assert_eq!(driver.closes.get(), 1);
}
