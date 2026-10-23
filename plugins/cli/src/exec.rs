//! A surface's Fipe session; each run owns its budget and resources.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use fipe::decl::Declarations;
use fipe::embed::{Outcome, Reply, Request};
use fipe::ir::Tables;
use fipe::session::{Diagnostic, Report, Session};
use fipe::token::Source;
use fipe::value::Value;
use fipe::vm::Machine;
use reovim_kernel_interface::{Calls, Decl, Error, Op, Out, Ty};
use reovim_sdk::dynamic::{self, Value as Wire};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

use crate::value::{Mapping, Resources};

struct Api {
    pub home: &'static str,
    pub decl: Decl,
    text: String,
}

pub struct Executor {
    pub session: Session,
    apis: Vec<Api>,
}

impl Executor {
    pub fn new() -> Result<Self, Error> {
        let apis = apis();
        let mut declarations = Declarations::new();
        declarations
            .commands(include_str!("../../../sdk/rust/commands.fipe"))
            .map_err(|_| Error::Invalid)?;
        for api in &apis {
            let name = format!("{}.{}", api.home, api.decl.api);
            declarations
                .module(&name, &api.text)
                .map_err(|_| Error::Invalid)?;
        }
        Ok(Self { session: Session::new(declarations), apis })
    }

    fn check(
        &mut self,
        ctx: &dyn Calls,
        path: &str,
        text: &str,
        provided: Option<(&str, &str)>,
    ) -> Report {
        let mut outcome = self.session.line(text);
        loop {
            let request = match outcome {
                Outcome::Done(report) => return report,
                Outcome::Request(request) => request,
            };
            let reply = checking_reply(ctx, path, request, provided);
            outcome = self.session.reply(reply);
        }
    }

    pub fn run(
        &mut self,
        ctx: &dyn Calls,
        path: &str,
        text: &str,
        budget: u64,
    ) -> Result<(Report, String), Error> {
        self.execute(ctx, path, text, None, budget)
    }

    pub fn load(
        &mut self,
        ctx: &dyn Calls,
        path: &str,
        text: &str,
        budget: u64,
    ) -> Result<(Report, String), Error> {
        let stem = stem(path)?;
        self.execute(
            ctx,
            path,
            &format!("use {stem}"),
            Some((stem, text)),
            budget,
        )
    }

    fn execute(
        &mut self,
        ctx: &dyn Calls,
        path: &str,
        text: &str,
        provided: Option<(&str, &str)>,
        budget: u64,
    ) -> Result<(Report, String), Error> {
        let mut report = self.check(ctx, path, text, provided);
        let mut output = String::new();
        for diagnostic in &report.diagnostics {
            output.push_str(&diagnostic_line(ctx, path, text, diagnostic));
        }
        let Some(unit) = &report.unit else { return Ok((report, output)) };
        let program = fipe::compile::compile(unit);
        let mut machine = Machine::new(Some(budget));
        let mut resources = Resources::new(ctx);
        let outcome = machine.run(&program);
        let result =
            self.drive(&program, &mut machine, &mut resources, outcome);
        let tables = program.tables.borrow();
        for (_, value) in
            machine.lines().iter().filter(|(_, value)| *value != Value::Unit)
        {
            output.push_str(&fipe::value::structural::show(value, &tables));
            output.push('\n');
        }
        let fault_message =
            result.as_ref().err().map(|fault| fault_line(path, fault, &tables));
        drop(tables);
        if let Some(message) = fault_message {
            report.accepted.and_then(|line| self.session.withdraw(line));
            output.push_str(&message);
            output.push('\n');
            let log = cell::resolve(ctx, ROOT, "/sys/log")?;
            ops::put(ctx, log, format!("fipe: {message}").into_bytes())?;
            report.unit = None;
        }
        Ok((report, output))
    }

    pub fn invoke(
        &self,
        ctx: &dyn Calls,
        export: &crate::export::Export,
        bytes: &[u8],
        budget: u64,
    ) -> Result<Vec<u8>, Error> {
        let program = &export.program;
        let input = &export.decl.methods[0].input;
        let Wire::Record(values) = dynamic::decode(input, bytes)? else {
            return Err(Error::Invalid);
        };
        let Ty::Record(_, fields) = input else { return Err(Error::Invalid) };
        let mut resources = Resources::new(ctx);
        let args = {
            let tables = program.tables.borrow();
            let mut mapping = Mapping {
                tables: &tables,
                home: "",
                resources: &mut resources,
            };
            fields
                .iter()
                .zip(values)
                .map(|(field, (_, value))| mapping.lift(&field.ty, value))
                .collect::<Result<Vec<_>, _>>()?
        };
        let input = match args.as_slice() {
            [] => Value::Unit,
            [value] => value.clone(),
            _ => Value::Tuple(alloc::rc::Rc::new(args)),
        };
        let mut machine = Machine::new(Some(budget));
        let outcome = machine
            .run_with_input(program, input)
            .map_err(|_| Error::Invalid)?;
        let result = self.drive(program, &mut machine, &mut resources, outcome);
        let value = match result {
            Ok(value) => value,
            Err(fault) => {
                let message =
                    fault_line(&export.path, &fault, &program.tables.borrow());
                let output = format!("{message}\n").into_bytes();
                let log = cell::resolve(ctx, ROOT, "/sys/log")?;
                ops::put(ctx, log, format!("fipe: {message}").into_bytes())?;
                return Ok(output);
            }
        };
        let tables = program.tables.borrow();
        let mapping =
            Mapping { tables: &tables, home: "", resources: &mut resources };
        let value = mapping.to_wire(&export.result, &value)?;
        let bytes = dynamic::encode(&export.result, &value)?;
        dynamic::show(&export.result, &bytes)
    }

    fn drive(
        &self,
        program: &fipe::mir::Program,
        machine: &mut Machine,
        resources: &mut Resources<'_>,
        mut outcome: Outcome<Result<Value, fipe::vm::Fault>>,
    ) -> Result<Value, fipe::vm::Fault> {
        loop {
            let request = match outcome {
                Outcome::Done(result) => return result,
                Outcome::Request(request) => request,
            };
            let reply =
                self.answer(&program.tables.borrow(), request, resources);
            outcome = machine.reply(program, reply);
        }
    }

    fn answer(
        &self,
        tables: &Tables,
        request: Request,
        resources: &mut Resources<'_>,
    ) -> Reply {
        let Request::Call { declaration, arguments, .. } = request else {
            return Reply::File(None);
        };
        let declared = &tables.declarations[declaration];
        let namespace = &tables.namespaces[declared.namespace];
        let result = self.call(
            tables,
            namespace,
            &declared.label,
            &arguments,
            resources,
        );
        Reply::Value(
            result
                .map_err(|e| format!("{namespace}.{}: {e:?}", declared.label)),
        )
    }

    fn call(
        &self,
        tables: &Tables,
        namespace: &str,
        name: &str,
        args: &[Value],
        resources: &mut Resources<'_>,
    ) -> Result<Value, Error> {
        let api = self
            .apis
            .iter()
            .find(|api| namespace == format!("{}.{}", api.home, api.decl.api))
            .ok_or(Error::Unsupported)?;
        if name == "at" {
            return at(api, args, resources);
        }
        let (handle, args) = args.split_first().ok_or(Error::Invalid)?;
        let h = resources.handle(handle)?;
        let method = api
            .decl
            .methods
            .iter()
            .find(|m| m.name == name)
            .ok_or(Error::Unsupported)?;
        let mut mapping = Mapping { tables, home: api.home, resources };
        let input = arguments(&mapping, &method.input, args)?;
        let input = dynamic::encode(&method.input, &input)?;
        let out = mapping.resources.ctx.submit(Op::Call {
            h,
            api: api.decl.hash(),
            method: method.number,
            input,
        });
        let Out::Bytes(out) = ops::ready(out)? else {
            return Err(Error::Invalid);
        };
        mapping.lift(&method.output, dynamic::decode(&method.output, &out)?)
    }
}

fn fault_line(path: &str, fault: &fipe::vm::Fault, tables: &Tables) -> String {
    let namespace = &tables.namespaces[fault.namespace];
    let reason = visible(&fault.reason.to_string());
    format!(
        "{path} [{namespace}:{}..{}]: {reason}",
        fault.span.start, fault.span.end
    )
}

fn visible(text: &str) -> String {
    let mut output = String::new();
    for ch in text.chars() {
        if ch.is_control() {
            output.extend(ch.escape_default());
        } else {
            output.push(ch);
        }
    }
    output
}

fn at(
    api: &Api,
    args: &[Value],
    resources: &mut Resources<'_>,
) -> Result<Value, Error> {
    let [Value::Text(path)] = args else { return Err(Error::Invalid) };
    let rd = cell::resolve(resources.ctx, ROOT, path)?;
    if !ops::info(resources.ctx, rd)?.apis.contains(&api.decl.hash()) {
        return Err(Error::Unsupported);
    }
    resources.open(rd)
}

fn checking_reply(
    ctx: &dyn Calls,
    path: &str,
    request: Request,
    provided: Option<(&str, &str)>,
) -> Reply {
    match request {
        Request::File { stem } => Reply::File(
            provided
                .filter(|(name, _)| *name == stem)
                .map(|(_, text)| text.to_string())
                .or_else(|| file(ctx, path, &stem)),
        ),
        Request::Call { .. } => {
            Reply::Value(Err("host call during checking".into()))
        }
    }
}

fn arguments(
    mapping: &Mapping<'_, '_>,
    ty: &Ty,
    args: &[Value],
) -> Result<Wire, Error> {
    match (ty, args) {
        (Ty::Unit, []) => Ok(Wire::Unit),
        (Ty::Record(_, fields), args) if fields.len() == args.len() => {
            let values = fields
                .iter()
                .zip(args)
                .map(|(f, v)| Ok((f.name.clone(), mapping.to_wire(&f.ty, v)?)));
            Ok(Wire::Record(values.collect::<Result<_, Error>>()?))
        }
        (ty, [value]) => mapping.to_wire(ty, value),
        _ => Err(Error::Invalid),
    }
}

pub fn stem(path: &str) -> Result<&str, Error> {
    let name = path.rsplit('/').next().ok_or(Error::Invalid)?;
    let (stem, extension) = name.rsplit_once('.').ok_or(Error::Invalid)?;
    if !extension.eq_ignore_ascii_case("fipe") {
        return Err(Error::Invalid);
    }
    Ok(stem)
}

pub fn read(ctx: &dyn Calls, path: &str) -> Result<String, Error> {
    let rd = cell::resolve(ctx, ROOT, path)?;
    String::from_utf8(ops::bytes(ctx, rd)?).map_err(|_| Error::Invalid)
}

fn file(ctx: &dyn Calls, path: &str, stem: &str) -> Option<String> {
    let (parent, _) = path.rsplit_once('/')?;
    read(ctx, &format!("{parent}/{stem}.fipe")).ok()
}

fn diagnostic_line(
    ctx: &dyn Calls,
    path: &str,
    text: &str,
    diagnostic: &Diagnostic,
) -> String {
    let (stem, span, message) = match diagnostic {
        Diagnostic::Syntax(e) => (None, e.span, e.reason.to_string()),
        Diagnostic::Resolve(e) => (None, e.span, e.reason.to_string()),
        Diagnostic::Type(e) => (None, e.span, e.reason.to_string()),
        Diagnostic::Compile { span, reason, .. } => {
            (None, *span, reason.to_string())
        }
        Diagnostic::File { stem, error } => {
            (Some(stem), error.span, error.reason.to_string())
        }
        Diagnostic::FileResolve { stem, error } => {
            (Some(stem), error.span, error.reason.to_string())
        }
        Diagnostic::FileType { stem, error } => {
            (Some(stem), error.span, error.reason.to_string())
        }
    };
    let imported = stem.and_then(|stem| file(ctx, path, stem));
    let source = imported.as_deref().unwrap_or(text);
    let (line, col) = Source::new(source).line_col(span.start);
    let name = stem.map_or_else(|| path.to_string(), |s| format!("{s}.fipe"));
    let message = visible(&message);
    format!("{name}:{line}:{col}: {message}\n")
}

fn apis() -> Vec<Api> {
    use reovim_client_api as client;
    use reovim_text_api as text;
    let mut apis = Vec::new();
    let mut add = |home, decl, text| apis.push(Api { home, decl, text });
    add("text", text::open::decl(), text::open::text("text"));
    add("text", text::text::decl(), text::text::text("text"));
    add("text", text::history::decl(), text::history::text("text"));
    add("text", text::publish::decl(), text::publish::text("text"));
    add(
        "search",
        reovim_search_api::search::decl(),
        reovim_search_api::search::text("search"),
    );
    add("client", client::keyboard::decl(), client::keyboard::text("client"));
    add("client", client::window::decl(), client::window::text("client"));
    add("client", client::display::decl(), client::display::text("client"));
    add("client", client::looks::decl(), client::looks::text("client"));
    add("client", client::open::decl(), client::open::text("client"));
    add("client", client::buffers::decl(), client::buffers::text("client"));
    add("client", client::draft::decl(), client::draft::text("client"));
    add("client", client::resolver::decl(), client::resolver::text("client"));
    add(
        "sdk",
        reovim_sdk::progress::progress::decl(),
        reovim_sdk::progress::progress::text("sdk"),
    );
    add("sdk", reovim_sdk::env::env::decl(), reovim_sdk::env::env::text("sdk"));
    apis
}

#[cfg(test)]
mod tests;
