//! Command line argument parsing.

pub(crate) struct ParsedArgs {
    pub(crate) files: Vec<String>,
    pub(crate) output: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) host: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) node: bool,
    pub(crate) force: bool,
}

pub(crate) fn parse_args(args: &[String]) -> Result<ParsedArgs, String> {
    let mut parsed = ParsedArgs {
        files: Vec::new(),
        output: None,
        port: None,
        host: None,
        title: None,
        node: false,
        force: false,
    };

    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "-o" | "--out" => parsed.output = Some(value_of(args, &mut index, arg)?),
            "--port" | "-p" => {
                let value = value_of(args, &mut index, arg)?;
                parsed.port = Some(
                    value
                        .parse()
                        .map_err(|_| format!("'{}' is not a port", value))?,
                );
            }
            "--host" => parsed.host = Some(value_of(args, &mut index, arg)?),
            "--title" => parsed.title = Some(value_of(args, &mut index, arg)?),
            "--node" => parsed.node = true,
            "--force" => parsed.force = true,
            _ => {
                if let Some(value) = arg.strip_prefix("--out=") {
                    parsed.output = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--port=") {
                    parsed.port = Some(
                        value
                            .parse()
                            .map_err(|_| format!("'{}' is not a port", value))?,
                    );
                } else if let Some(value) = arg.strip_prefix("--host=") {
                    parsed.host = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--title=") {
                    parsed.title = Some(value.to_string());
                } else if arg.starts_with('-') && arg.len() > 1 {
                    return Err(format!("unknown option '{}'", arg));
                } else {
                    parsed.files.push(arg.clone());
                }
            }
        }
        index += 1;
    }

    Ok(parsed)
}

/// The value of an option.
fn value_of(args: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    *index += 1;
    match args.get(*index) {
        Some(value) => Ok(value.clone()),
        None => Err(format!("'{}' needs a value", option)),
    }
}
