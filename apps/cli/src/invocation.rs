use std::io;

pub struct Invocation<'a> {
    pub path: &'a str,
    pub args: &'a [&'a str],
    pub contract: Option<(&'a str, u32)>,
    pub hex: Option<&'a str>,
    pub references: Option<&'a str>,
}

pub fn parse<'a>(mut fields: &'a [&'a str]) -> io::Result<Invocation<'a>> {
    let mut invocation = Invocation {
        path: "",
        args: &[],
        contract: None,
        hex: None,
        references: None,
    };
    while fields.first().is_some_and(|field| field.starts_with("--")) {
        fields = option(&mut invocation, fields)?;
    }
    let Some((path, args)) = fields.split_first() else {
        return Err(io::Error::other("expected a Resource path"));
    };
    if invocation.hex.is_some() && !args.is_empty() {
        return Err(io::Error::other(
            "hex data already supplies the complete input",
        ));
    }
    if invocation.contract.is_some() && !args.is_empty() {
        return Err(io::Error::other(
            "explicit contracts use --hex or empty data",
        ));
    }
    invocation.path = path;
    invocation.args = args;
    Ok(invocation)
}

fn option<'a>(
    value: &mut Invocation<'a>,
    fields: &'a [&'a str],
) -> io::Result<&'a [&'a str]> {
    match fields {
        ["--contract", name, version, rest @ ..]
            if value.contract.is_none() =>
        {
            value.contract =
                Some((name, version.parse().map_err(io::Error::other)?));
            Ok(rest)
        }
        ["--hex", data, rest @ ..] if value.hex.is_none() => {
            value.hex = Some(data);
            Ok(rest)
        }
        ["--refs", references, rest @ ..] if value.references.is_none() => {
            value.references = Some(references);
            Ok(rest)
        }
        _ => Err(io::Error::other("invalid or repeated invocation option")),
    }
}

impl Invocation<'_> {
    pub fn needs_cwd(&self) -> bool {
        (relative(self.path)
            && (self.path.contains('/') || matches!(self.path, "." | "..")))
            || self
                .references
                .is_some_and(|references| references.split(',').any(relative))
    }
}
fn relative(path: &str) -> bool {
    !path.starts_with('/') && !path.starts_with('@')
}
