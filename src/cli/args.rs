//! Command line argument parsing.

pub(crate) struct ParsedArgs {
    pub(crate) files: Vec<String>,
    pub(crate) output: Option<String>,
    pub(crate) port: Option<u16>,
    pub(crate) host: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) node: bool,
    /// The user asked for the help of the command.
    pub(crate) help: bool,
    /// The name of the tests that are run.
    pub(crate) filter: Option<String>,
    /// The command runs again when a file changes.
    pub(crate) watch: bool,
    /// The shape of the new project.
    pub(crate) template: Option<String>,
    /// Where the site is put.
    pub(crate) target: Option<String>,
    /// The files are written even when they are there.
    pub(crate) write: bool,
    /// The command writes over what is there.
    pub(crate) force: bool,
}

impl ParsedArgs {
    pub(crate) fn new() -> Self {
        Self {
            files: Vec::new(),
            output: None,
            port: None,
            host: None,
            title: None,
            node: false,
            help: false,
            filter: None,
            watch: false,
            template: None,
            target: None,
            write: false,
            force: false,
        }
    }
}

pub(crate) fn parse_args(args: &[String]) -> Result<ParsedArgs, String> {
    let mut parsed = ParsedArgs::new();

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
            "-h" | "--help" => parsed.help = true,
            "--filter" => parsed.filter = Some(value_of(args, &mut index, arg)?),
            "--watch" => parsed.watch = true,
            "--template" => parsed.template = Some(value_of(args, &mut index, arg)?),
            "--target" => parsed.target = Some(value_of(args, &mut index, arg)?),
            "--write" => parsed.write = true,
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
                } else if let Some(value) = arg.strip_prefix("--filter=") {
                    parsed.filter = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--template=") {
                    parsed.template = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--target=") {
                    parsed.target = Some(value.to_string());
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

/// Whether the `--force` mark is in the arguments.
///
/// Only the commands that write over what is there read it.
pub(crate) fn wants_force(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--force")
}
/// The value of an option.
fn value_of(args: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    *index += 1;
    match args.get(*index) {
        Some(value) => Ok(value.clone()),
        None => Err(format!("'{}' needs a value", option)),
    }
}
