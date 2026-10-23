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

pub(crate) fn parse_args(command: &str, args: &[String]) -> Result<ParsedArgs, String> {
    let mut parsed = ParsedArgs::new();

    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "-o" | "--out" => {
                allow(command, "--out")?;
                parsed.output = Some(value_of(args, &mut index, arg)?)
            }
            "--port" | "-p" => {
                allow(command, "--port")?;
                let value = value_of(args, &mut index, arg)?;
                parsed.port = Some(
                    value
                        .parse()
                        .map_err(|_| format!("'{}' is not a port", value))?,
                );
            }
            "--host" => {
                allow(command, "--host")?;
                parsed.host = Some(value_of(args, &mut index, arg)?)
            }
            "--title" => {
                allow(command, "--title")?;
                parsed.title = Some(value_of(args, &mut index, arg)?)
            }
            "--node" => {
                allow(command, "--node")?;
                parsed.node = true;
            }
            "-h" | "--help" => parsed.help = true,
            "--filter" => {
                allow(command, "--filter")?;
                parsed.filter = Some(value_of(args, &mut index, arg)?)
            }
            "--watch" => {
                allow(command, "--watch")?;
                parsed.watch = true;
            }
            "--template" => {
                allow(command, "--template")?;
                parsed.template = Some(value_of(args, &mut index, arg)?)
            }
            "--target" => {
                allow(command, "--target")?;
                parsed.target = Some(value_of(args, &mut index, arg)?)
            }
            "--write" => {
                allow(command, "--write")?;
                parsed.write = true;
            }
            "--force" => {
                allow(command, "--force")?;
                parsed.force = true;
            }
            _ => {
                if let Some(value) = arg.strip_prefix("--out=") {
                    allow(command, "--out")?;
                    parsed.output = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--port=") {
                    allow(command, "--port")?;
                    parsed.port = Some(
                        value
                            .parse()
                            .map_err(|_| format!("'{}' is not a port", value))?,
                    );
                } else if let Some(value) = arg.strip_prefix("--host=") {
                    allow(command, "--host")?;
                    parsed.host = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--title=") {
                    allow(command, "--title")?;
                    parsed.title = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--filter=") {
                    allow(command, "--filter")?;
                    parsed.filter = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--template=") {
                    allow(command, "--template")?;
                    parsed.template = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--target=") {
                    allow(command, "--target")?;
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

/// Whether a command uses an option.
///
/// An option that belongs to another command is an error, so that a typo or a
/// copied command line does not change nothing without saying so.
fn allow(command: &str, option: &str) -> Result<(), String> {
    let allowed = match option {
        "--out" => matches!(command, "build" | "docs" | "deploy"),
        "--port" | "--host" => matches!(command, "run" | "dev"),
        "--title" => matches!(command, "run" | "dev" | "build" | "deploy"),
        "--node" => command == "build",
        "--filter" | "--watch" => command == "test",
        "--template" => command == "new",
        "--target" => command == "deploy",
        "--write" => matches!(command, "fmt" | "migrate"),
        "--force" => matches!(command, "build" | "new" | "deploy"),
        _ => false,
    };
    if allowed {
        Ok(())
    } else {
        Err(format!("unknown option '{}' for 'vey {}'", option, command))
    }
}
/// The value of an option.
fn value_of(args: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    *index += 1;
    match args.get(*index) {
        Some(value) => Ok(value.clone()),
        None => Err(format!("'{}' needs a value", option)),
    }
}
