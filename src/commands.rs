use clap::{Arg, ArgMatches, Command};

pub fn create_cli_commands() -> ArgMatches {
    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .about(env!("CARGO_PKG_DESCRIPTION"))
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("login")
                .about("initiate new session")
        )
        // working with configuration peroperties
        .subcommand(
            Command::new("config")
                .about("working with configuration")
                .subcommand(
                    Command::new("list")
                        .about("shows the list of all configuration properties and their values"),
                )
                .subcommand(
                    Command::new("show-default-location")
                        .about("shows the path to the default configuration file"),
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
                )
                .subcommand(
                    Command::new("tenant")
                        .about("working with tenant configuration")
                        .subcommand(
                            Command::new("add")
                                .about("adds new tenant configuration")
                                .arg(
                                    Arg::new("alias")
                                        .long("alias")
                                        .required(false)
                                        .help("tenant's alias for this configuration. It is optional. If not provided, the tenant ID will be used instead")
                                )
                                .arg(
                                    Arg::new("id")
                                        .long("id")
                                        .required(true)
                                        .help("tenant's identifier")
                                )
                                .arg(
                                    Arg::new("api-url")
                                        .long("api-url")
                                        .required(true)
                                        .help("tenants API base URL")
                                )
                                .arg(
                                    Arg::new("oidc-url")
                                        .long("oidc-url")
                                        .required(true)
                                        .help("OpenID Connect authorization URL")
                                )
                                .arg(
                                    Arg::new("client-id")
                                        .long("client-id")
                                        .required(true)
                                        .help("OpenID Connect client ID")
                                )
                                .arg(
                                    Arg::new("client-secret")
                                        .long("client-secret")
                                        .required(true)
                                        .help("OpenID Connect client secret")
                                )
                        )
                        .subcommand(
                            Command::new("delete")
                                .about("deletes a tenant configuration")
                                .arg(
                                    Arg::new("id")
                                        .long("id")
                                        .required(true)
                                        .help("the tenants ID or alias")    
                                )
                        )
                        .subcommand(
                            Command::new("show-all-aliases")
                                .about("lists all configured tenants")
                        )
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
