use clap::{Arg, ArgMatches, Command};

pub fn create_cli_commands() -> ArgMatches {
    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        // working with configuration peroperties
        .subcommand(
            Command::new("config")
                .about("working with configuration")
                .subcommand(
                    Command::new("list")
                        .about("shows the list of all configuration properties and their values"),
                )
                .subcommand(
                    Command::new("init")
                        .about("initializes with a new defult configuration")
                        .arg(
                            Arg::new("file")
                                .short('f')
                                .long("file")
                                .required(true)
                                .help("configuration file path"),
                        ),
                )
                .subcommand(
                    Command::new("import")
                        .about("imports configuration from a given file into the default configuraiton file")
                        .arg(
                            Arg::new("file")
                                .short('f')
                                .long("file")
                                .required(true)
                                .help("configuration file path"),
                        ),
                )
                .subcommand(
                    Command::new("export")
                        .about("saves the default configuration file to a different file")
                        .arg(
                            Arg::new("file")
                                .short('f')
                                .long("file")
                                .required(true)
                                .help("configuration file path"),
                        ),
                )
                .subcommand(
                    Command::new("show-names")
                        .about("prints all valid configuration property names"),
                )
                .subcommand(
                    Command::new("get")
                        .about("returns the value for a configuration property")
                        .arg(
                            Arg::new("name")
                                .short('n')
                                .long("name")
                                .required(true)
                                .help("property name"),
                        ),
                )
                .subcommand(
                    Command::new("set")
                        .about("sets the value for a configuration property")
                        .arg(
                            Arg::new("name")
                                .short('n')
                                .long("name")
                                .required(true)
                                .help("property name"),
                        )
                        .arg(
                            Arg::new("value")
                                .short('v')
                                .long("value")
                                .required(true)
                                .help("property value"),
                        ),
                )
                .subcommand(
                    Command::new("delete")
                        .about("deletes a configuration peroperty")
                        .arg(
                            Arg::new("name")
                                .short('n')
                                .long("name")
                                .required(true)
                                .help("deletes a configuration property"),
                        ),
                ),
        )
        //working with folders
        .subcommand(
            Command::new("folder")
                .about("working with folders")
                .subcommand(
                    Command::new("get")
                        .about("reads list of folders matching the search clause")
                        .arg(
                            Arg::new("search")
                                .short('s')
                                .long("search")
                                .required(false)
                                .help("search clause"),
                        ),
                )
                .subcommand(
                    Command::new("add")
                        .about("creates new folder with the provided name")
                        .arg(
                            Arg::new("name")
                                .short('n')
                                .long("name")
                                .required(true)
                                .help("name of the new folder"),
                        ),
                ),
        )
        .get_matches()
}
