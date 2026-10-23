//! Fipe host declarations without a language runtime dependency.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use crate::{Decl, Field, Method, Rights, Ty};

impl Decl {
    /// Render this api as Fipe host-module source.
    ///
    /// Named records and choices carry `api.Type` identities. Bounds stay
    /// in the wire contract. The api version is a module doc; each
    /// method documents its required right.
    /// An unrefined wire `Rd` is `Rd<()>`; a method's handle spot is
    /// `Rd<module>`. The unit refinement declares no api.
    ///
    /// ```
    /// use reovim_kernel_interface::Decl;
    /// let decl = Decl {
    ///     api: "example".into(), ver: 1, methods: vec![], records: vec![],
    /// };
    /// assert!(decl.render().contains("pub at: (path: String)"));
    /// ```
    #[must_use]
    pub fn render(&self) -> String {
        self.render_in("")
    }

    /// Render under a host domain, using `home.api` module identities.
    /// References to other apis use the same host domain.
    ///
    /// ```
    /// use reovim_kernel_interface::{Decl, Method, Rights, Ty};
    /// let decl = Decl {
    ///     api: "open".into(), ver: 1, records: vec![],
    ///     methods: vec![Method {
    ///         number: 0, name: "open".into(),
    ///         input_name: Some("source".into()), input: Ty::Rd,
    ///         output: Ty::Rd, right: Rights::WRITE,
    ///     }],
    /// };
    /// assert_eq!(decl.render_in("text"), concat!(
    ///     "##! open api, version 1.\n",
    ///     "## The resource at `path` that offers this api.\n",
    ///     "pub at: (path: String) -> Rd<text.open>\n",
    ///     "## Needs WRITE.\n",
    ///     "pub open: (h: Rd<text.open>) -> (source: Rd<()>) -> Rd<()>\n",
    /// ));
    /// ```
    #[must_use]
    pub fn render_in(&self, home: &str) -> String {
        render(self, home)
    }
}

fn render(decl: &Decl, home: &str) -> String {
    let mut out = format!("##! {} api, version {}.\n", decl.api, decl.ver);
    let _ = writeln!(
        out,
        "## The resource at `path` that offers this api.\n\
         pub at: (path: String) -> Rd<{}>",
        module_name(home, &decl.api)
    );
    let mut types = Vec::new();
    for (name, fields) in &decl.records {
        collect(
            &Ty::Record(format!("{}.{name}", decl.api), fields.clone()),
            &mut types,
        );
    }
    for method in &decl.methods {
        collect(&method.input, &mut types);
        collect(&method.output, &mut types);
    }
    let mut imports = BTreeSet::new();
    for ty in &types {
        let (name, _, _) = named(ty);
        if let Some((api, _)) = name.rsplit_once('.')
            && api != decl.api
        {
            imports.insert(api);
        }
    }
    for api in imports {
        let _ = writeln!(out, "use {}", module_name(home, api));
    }
    for ty in &types {
        render_type(decl, &mut out, ty, home);
    }
    for method in &decl.methods {
        render_method(decl, &mut out, method, home);
    }
    out
}

fn render_method(decl: &Decl, out: &mut String, method: &Method, home: &str) {
    let right = if method.right == Rights::WRITE { "WRITE" } else { "READ" };
    let _ = writeln!(out, "## Needs {right}.");
    let _ = write!(
        out,
        "pub {}: (h: Rd<{}>)",
        method.name,
        module_name(home, &decl.api)
    );
    match &method.input {
        Ty::Unit => {}
        Ty::Record(_, fields) => {
            for field in fields {
                let _ = write!(
                    out,
                    " -> ({}: {})",
                    field.name,
                    type_name(decl, &field.ty, home)
                );
            }
        }
        ty => {
            let name = method.input_name.as_deref().unwrap_or("input");
            let _ = write!(out, " -> ({name}: {})", type_name(decl, ty, home));
        }
    }
    let _ = writeln!(out, " -> {}", type_name(decl, &method.output, home));
}

fn type_name(decl: &Decl, ty: &Ty, home: &str) -> String {
    match ty {
        Ty::Unit => "()".into(),
        Ty::Bool => "Bool".into(),
        Ty::U8 => "U8".into(),
        Ty::U32 => "U32".into(),
        Ty::U64 => "U64".into(),
        Ty::I64 => "I64".into(),
        Ty::F64 => "F64".into(),
        Ty::Rd => "Rd<()>".into(),
        Ty::Utf8(_) | Ty::Rest(_) => "String".into(),
        Ty::Bytes(_) => "Vec<U8>".into(),
        Ty::Sequence(_, item) => {
            format!("Vec<{}>", type_name(decl, item, home))
        }
        Ty::Array(n, item) => format!("[{}; {n}]", type_name(decl, item, home)),
        Ty::Record(name, _) | Ty::Choice(name, _) => {
            match name.rsplit_once('.') {
                Some((api, name)) if api == decl.api => name.into(),
                Some((api, name)) => {
                    // shortcut: one home, map imports when domains mix.
                    format!("{}.{name}", module_name(home, api))
                }
                None => name.clone(),
            }
        }
    }
}

fn render_type(decl: &Decl, out: &mut String, ty: &Ty, home: &str) {
    let (name, fields, choice) = named(ty);
    if name.rsplit_once('.').is_some_and(|(api, _)| api != decl.api) {
        return;
    }
    let _ = writeln!(out, "## The {} type.", type_name(decl, ty, home));
    let _ = writeln!(out, "pub type {} {{", type_name(decl, ty, home));
    for field in fields {
        render_field(decl, out, field, home, choice);
    }
    out.push_str("}\n");
}

fn render_field(
    decl: &Decl,
    out: &mut String,
    field: &Field,
    home: &str,
    choice: bool,
) {
    let ty = type_name(decl, &field.ty, home);
    let part = if choice { "variant" } else { "field" };
    let _ = writeln!(out, "    ## The {} {part}.", field.name);
    if choice {
        let _ = writeln!(out, "    {}(value: {ty}),", field.name);
    } else {
        let _ = writeln!(out, "    pub {}: {ty},", field.name);
    }
}

fn named(ty: &Ty) -> (&str, &[Field], bool) {
    match ty {
        Ty::Record(name, fields) => (name, fields, false),
        Ty::Choice(name, fields) => (name, fields, true),
        _ => unreachable!("collected named types only"),
    }
}

fn collect(ty: &Ty, types: &mut Vec<Ty>) {
    match ty {
        Ty::Record(_, fields) | Ty::Choice(_, fields) => {
            if types.contains(ty) {
                return;
            }
            types.push(ty.clone());
            for field in fields {
                collect(&field.ty, types);
            }
        }
        Ty::Array(_, item) | Ty::Sequence(_, item) => collect(item, types),
        _ => {}
    }
}

fn module_name(home: &str, api: &str) -> String {
    if home.is_empty() { api.into() } else { format!("{home}.{api}") }
}
