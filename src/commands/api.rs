//! `pcli2 api`: send any request to the Physna API.

use clap::{Arg, ArgAction, Command};

use crate::commands::params::{tenant_parameter, PARAMETER_INPUT};

pub fn api_command() -> Command {
    Command::new("api")
        .about("Send a request to any Physna API endpoint, with pcli2's login, retries and tenant")
        .long_about(
            "Send a request to any endpoint of the Physna API and print the response body.\n\n\
             The path is relative to the API URL of the environment (for example \
             /tenants/{tenantId}/folders); {tenantId} is replaced with the active tenant, or \
             the one named with --tenant. The request is authenticated like every pcli2 \
             command, renews an expired token, and retries transient failures.\n\n\
             A non-2xx answer is an error with the server's message and the usual exit code \
             (67 not found, 102 rejected, 69 try again later, ...).",
        )
        .after_help(crate::commands::examples(&[
            ("The signed-in user and their tenants", "pcli2 api /users/me"),
            ("The first page of folders of the active tenant", "pcli2 api /tenants/{tenantId}/folders"),
            ("Every page, merged into one list", "pcli2 api /tenants/{tenantId}/folders --paginate"),
            (
                "A POST with a JSON body built from fields",
                "pcli2 api /tenants/{tenantId}/assets/existing-paths -X POST -F paths=[\"/Home/a.stl\"]",
            ),
            ("A body from a file", "pcli2 api /tenants/{tenantId}/reports/duplication -X POST --input request.json"),
        ]))
        .arg(
            Arg::new("path")
                .required(true)
                .help("Endpoint path, relative to the API URL; {tenantId} is filled in"),
        )
        .arg(
            Arg::new("method")
                .short('X')
                .long("method")
                .num_args(1)
                .ignore_case(true)
                .value_parser(["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD"])
                .help("HTTP method (default: GET, or POST when a body is given)"),
        )
        .arg(
            Arg::new("field")
                .short('F')
                .long("field")
                .num_args(1)
                .action(ArgAction::Append)
                .value_name("KEY=VALUE")
                .help("Add a field to the JSON body; the value is read as JSON when it parses (numbers, true, [..], {..}), else as a string"),
        )
        .arg(
            Arg::new("raw-field")
                .short('f')
                .long("raw-field")
                .num_args(1)
                .action(ArgAction::Append)
                .value_name("KEY=VALUE")
                .help("Add a field to the JSON body, always as a string"),
        )
        .arg(
            Arg::new(PARAMETER_INPUT)
                .long(PARAMETER_INPUT)
                .num_args(1)
                .value_name("FILE")
                .conflicts_with_all(["field", "raw-field"])
                .help("The JSON body, from a file (- for standard input)"),
        )
        .arg(
            Arg::new("paginate")
                .long("paginate")
                .action(ArgAction::SetTrue)
                .help("For a GET listing: fetch every page and print one list"),
        )
        .arg(tenant_parameter())
}
