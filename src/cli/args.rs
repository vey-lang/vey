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

pub(crate) fn parse_args(args: &[String]) -> ParsedArgs {
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
            "-o" | "--out" => {
                index += 1;
                if index < args.len() {
                    parsed.output = Some(args[index].clone());
                }
            }
            "--port" | "-p" => {
                index += 1;
                if index < args.len() {
                    parsed.port = args[index].parse().ok();
                }
            }
            "--host" => {
                index += 1;
                if index < args.len() {
                    parsed.host = Some(args[index].clone());
                }
            }
            "--title" => {
                index += 1;
                if index < args.len() {
                    parsed.title = Some(args[index].clone());
                }
            }
            "--node" => parsed.node = true,
            "--force" => parsed.force = true,
            _ => {
                if let Some(value) = arg.strip_prefix("--out=") {
                    parsed.output = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--port=") {
                    parsed.port = value.parse().ok();
                } else if let Some(value) = arg.strip_prefix("--host=") {
                    parsed.host = Some(value.to_string());
                } else if let Some(value) = arg.strip_prefix("--title=") {
                    parsed.title = Some(value.to_string());
                } else {
                    parsed.files.push(arg.clone());
                }
            }
        }
        index += 1;
    }

    parsed
}
