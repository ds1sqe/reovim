//! Checked public functions exposed as ordinary shell commands.

use alloc::boxed::Box;
use alloc::collections::BTreeSet;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use fipe::ast::{Entry, ExprKind, PatternKind};
use fipe::ir::Tables;
use fipe::session::Session;
use fipe::typed::Typed;
use fipe::types::{Type, TypeName};
use reovim_kernel_interface::{Decl, Error, Field, Method, Rights, Ty};

#[derive(Clone)]
pub struct Export {
    pub decl: Decl,
    pub result: Ty,
    pub program: Rc<fipe::mir::Program>,
    pub path: String,
}

pub fn exports(
    session: &Session,
    path: &str,
    stem: &str,
    source: &str,
) -> Result<Vec<Export>, String> {
    let module = fipe::parse::parse(fipe::lex::lex(source))
        .map_err(|_| "invalid module syntax")?;
    let mut exports = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in module.entries.iter().rev() {
        let Entry::Line(id) = entry else { continue };
        let Some(expr) = module.expr(*id) else { continue };
        let ExprKind::Bind { public, pattern, value, .. } = &expr.kind else {
            continue;
        };
        let Some(pattern) = module.pattern(*pattern) else { continue };
        let PatternKind::Name(name) = &pattern.kind else { continue };
        let name = name.segments.last().ok_or("missing function name")?;
        if !seen.insert(name.clone()) || !public {
            continue;
        }
        let qualified = format!("{stem}.{name}");
        let report = inspect(session, &qualified)?;
        let unit = report.unit.as_ref().ok_or("no checked unit")?;
        let tables = unit.tables();
        let typed = report.typed.as_ref().ok_or("no checked types")?;
        let ty = exported_type(&report).ok_or("no exported type")?;
        if !matches!(ty, Type::Function { .. }) {
            continue;
        }
        let labels = parameters(&module, *value)
            .map_err(|_| format!("{name}: parameters must have names"))?;
        let export =
            function(session, path, &qualified, labels, ty, typed, &tables)?;
        exports.push(export);
    }
    exports.reverse();
    Ok(exports)
}

fn exported_type(report: &fipe::session::Report) -> Option<&Type> {
    let typed = report.typed.as_ref()?;
    let Entry::Line(id) = report.entries().last()? else { return None };
    let span = report.module.as_ref()?.expr(*id)?.span;
    // Defaults can invoke a bare name; keep its unapplied type.
    typed.callee_types.get(&(span.start, span.end)).or(typed.main.as_ref())
}

fn inspect(
    session: &Session,
    name: &str,
) -> Result<fipe::session::Report, String> {
    let mut probe = session.clone();
    let fipe::embed::Outcome::Done(report) = probe.query(name) else {
        return Err(format!("{name}: an export requested an import"));
    };
    if !report.diagnostics.is_empty() {
        return Err(format!("{name}: an export did not check"));
    }
    Ok(report)
}

fn parameters(
    module: &fipe::ast::Module,
    value: fipe::ast::ExprId,
) -> Result<Vec<String>, Error> {
    let expr = module.expr(value).ok_or(Error::Invalid)?;
    let ExprKind::Arms(arms) = &expr.kind else { return Err(Error::Invalid) };
    let arm = arms.first().ok_or(Error::Invalid)?;
    arm.patterns
        .iter()
        .map(|id| {
            let pattern = module.pattern(*id).ok_or(Error::Invalid)?;
            match &pattern.kind {
                PatternKind::Name(name) | PatternKind::Default { name, .. } => {
                    name.segments.last().cloned().ok_or(Error::Invalid)
                }
                _ => Err(Error::Invalid),
            }
        })
        .collect()
}

fn function(
    session: &Session,
    path: &str,
    api: &str,
    labels: Vec<String>,
    ty: &Type,
    typed: &Typed,
    tables: &Tables,
) -> Result<Export, String> {
    let mut fields = Vec::new();
    let mut ty = ty;
    for label in labels {
        let Type::Function { parameter, result } = ty else {
            return Err(format!("{api}: {label}: no parameter type"));
        };
        let field_type = wire(parameter, typed, tables, 0).map_err(|_| {
            format!("{api}: {label}: unmappable parameter type")
        })?;
        fields.push(Field { name: label, ty: field_type });
        ty = result;
    }
    let result = wire(ty, typed, tables, 0)
        .map_err(|_| format!("{api}: unmappable result type"))?;
    let args: Vec<String> =
        (0..fields.len()).map(|i| format!("arg{i}")).collect();
    let pattern = if args.len() == 1 {
        args[0].clone()
    } else if args.is_empty() {
        "ignored".into()
    } else {
        format!("({})", args.join(", "))
    };
    let expression = format!("{pattern} => {api} {}", args.join(" "));
    let decl = Decl {
        api: api.into(),
        ver: 1,
        records: Vec::new(),
        methods: alloc::vec![Method {
            number: 0,
            name: "run".into(),
            input_name: None,
            input: Ty::Record(format!("{api}.Args"), fields),
            output: Ty::Rd,
            right: Rights::READ,
        }],
    };
    decl.check()
        .map_err(|_| format!("{api}: declaration exceeds wire bounds"))?;
    let mut invocation = session.clone();
    let fipe::embed::Outcome::Done(report) = invocation.line(&expression)
    else {
        return Err(format!("{api}: invocation requested an import"));
    };
    let unit = report
        .unit
        .as_ref()
        .ok_or_else(|| format!("{api}: invocation did not check"))?;
    let program = Rc::new(fipe::compile::compile(unit));
    Ok(Export { decl, result, program, path: path.to_string() })
}

fn wire(
    ty: &Type,
    typed: &Typed,
    tables: &Tables,
    depth: usize,
) -> Result<Ty, Error> {
    if depth > 16 {
        return Err(Error::Invalid);
    }
    match ty {
        Type::Unit => Ok(Ty::Unit),
        Type::Limited { name, .. } => wire(name, typed, tables, depth + 1),
        Type::Array { element, length } => {
            let Type::Number(n) = &**length else { return Err(Error::Invalid) };
            let n = n
                .to_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or(Error::Invalid)?;
            Ok(Ty::Array(n, Box::new(wire(element, typed, tables, depth + 1)?)))
        }
        Type::Named { name, arguments } => {
            named(name, arguments, typed, tables, depth)
        }
        _ => Err(Error::Invalid),
    }
}

fn named(
    name: &TypeName,
    args: &[Type],
    typed: &Typed,
    tables: &Tables,
    depth: usize,
) -> Result<Ty, Error> {
    use fipe::resolve::Origin;
    if name.origin == Origin::Prelude {
        return primitive(&name.name, args, typed, tables, depth);
    }
    if name.origin == Origin::Commands && name.name == "Rd" {
        return Ok(Ty::Rd);
    }
    if !args.is_empty() {
        return Err(Error::Invalid);
    }
    let shape = typed
        .shapes
        .iter()
        .find(|s| matches!(&s.ty, Type::Named { name: n, .. } if n == name))
        .ok_or(Error::Invalid)?;
    let meaning = tables
        .meanings
        .iter()
        .rev()
        .find(|m| {
            m.label == name.name
                && origin_matches(&name.origin, &tables.namespaces[m.namespace])
        })
        .ok_or(Error::Invalid)?;
    let namespace = &tables.namespaces[meaning.namespace];
    let qualified = format!("{namespace}.{}", meaning.label);
    if let [constructor] = shape.constructors.as_slice()
        && constructor.name == name.name
    {
        let fields = constructor
            .fields
            .iter()
            .map(|f| {
                Ok(Field {
                    name: f.name.clone(),
                    ty: wire(&f.ty, typed, tables, depth + 1)?,
                })
            })
            .collect::<Result<_, Error>>()?;
        return Ok(Ty::Record(qualified, fields));
    }
    let fields = shape
        .constructors
        .iter()
        .map(|c| {
            let ty = match c.fields.as_slice() {
                [] => Ty::Unit,
                [field] => wire(&field.ty, typed, tables, depth + 1)?,
                _ => return Err(Error::Invalid),
            };
            Ok(Field { name: c.name.clone(), ty })
        })
        .collect::<Result<_, Error>>()?;
    Ok(Ty::Choice(qualified, fields))
}

fn origin_matches(origin: &fipe::resolve::Origin, namespace: &str) -> bool {
    use fipe::resolve::Origin;
    match origin {
        Origin::This => namespace == "main" || namespace.starts_with("line "),
        Origin::Host(name) | Origin::File(name) => namespace == name,
        Origin::Line(n) => namespace == format!("line {}", n + 1),
        Origin::Prelude => namespace == "prelude",
        Origin::Commands => namespace == "commands",
    }
}

fn primitive(
    name: &str,
    args: &[Type],
    typed: &Typed,
    tables: &Tables,
    depth: usize,
) -> Result<Ty, Error> {
    Ok(match name {
        "Bool" => Ty::Bool,
        "U8" => Ty::U8,
        "U32" => Ty::U32,
        "U64" => Ty::U64,
        "I64" | "Integer" => Ty::I64,
        "F64" => Ty::F64,
        "String" => Ty::Utf8(4096),
        "Vec" => {
            let [item] = args else { return Err(Error::Invalid) };
            Ty::Sequence(256, Box::new(wire(item, typed, tables, depth + 1)?))
        }
        _ => return Err(Error::Invalid),
    })
}

#[cfg(test)]
mod tests;
