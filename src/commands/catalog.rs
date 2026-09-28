//! Catalog command definitions.
//!
//! A catalog is a tenant Physna has made searchable from other tenants, such
//! as a supplier catalog; it is enabled per customer. These commands match an
//! asset from one of your tenants against a catalog. They are a group of
//! their own, apart from `asset geometric-match` and friends, so nothing here
//! can change what those print.

use clap::{Arg, ArgMatches, Command};

use crate::commands::params::{
    asset_identifier_group, format_parameter, format_pretty_parameter,
    format_with_headers_parameter, path_parameter, tenant_parameter, threshold_parameter,
    uuid_parameter, COMMAND_LIST, PARAMETER_LIMIT,
};

pub const COMMAND_CATALOG: &str = "catalog";
pub const COMMAND_GEOMETRIC_MATCH: &str = "geometric-match";
pub const COMMAND_PART_MATCH: &str = "part-match";
pub const COMMAND_VISUAL_MATCH: &str = "visual-match";
pub const PARAMETER_IN: &str = "in";

fn output_format_args(cmd: Command) -> Command {
    cmd.arg(format_parameter().value_parser(["json", "csv", "table"]))
        .arg(format_pretty_parameter())
        .arg(format_with_headers_parameter())
}

/// `--in CATALOG`: only catalogs listed by `catalog list` are accepted.
fn in_parameter() -> Arg {
    Arg::new(PARAMETER_IN)
        .long(PARAMETER_IN)
        .num_args(1)
        .value_name("CATALOG")
        .env("PCLI2_CATALOG")
        .help("The catalog to search, by name or ID, from 'catalog list' (default: your only catalog)")
}

/// `--limit` for the match commands: a catalog can be large.
fn match_limit_parameter() -> Arg {
    Arg::new(PARAMETER_LIMIT)
        .long(PARAMETER_LIMIT)
        .num_args(1)
        .default_value("100")
        .value_parser(clap::value_parser!(usize))
        .help("Maximum number of matches to return")
}

fn match_command(name: &'static str, aliases: [&'static str; 2], about: &'static str) -> Command {
    output_format_args(
        Command::new(name)
            .visible_aliases(aliases)
            .about(about)
            .arg(tenant_parameter().help(
                "Tenant that holds the asset, by ID or short name (default: the active tenant)",
            ))
            .arg(uuid_parameter().help("The asset's UUID"))
            .arg(path_parameter().help("The asset's path (e.g., /Home/Parts/bracket.stl)"))
            .arg(in_parameter())
            .arg(match_limit_parameter())
            .group(asset_identifier_group()),
    )
}

/// Define the catalog command and its subcommands.
pub fn catalog_command() -> Command {
    Command::new(COMMAND_CATALOG)
        .about("Search a catalog, such as a supplier catalog, for matches of your assets")
        .long_about(
            "Search a catalog, such as a supplier catalog, for matches of your assets.\n\n\
             A catalog is a tenant Physna makes searchable from other tenants; it is enabled \
             per customer. 'catalog list' shows the catalogs you can search. Only those are \
             accepted by --in: a search never reaches another of your tenants, so results \
             from one company's tenant cannot answer a search from another's.",
        )
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(output_format_args(
            Command::new(COMMAND_LIST)
                .visible_alias("ls")
                .about("List the catalogs you can search"),
        ))
        .subcommand(
            match_command(
                COMMAND_GEOMETRIC_MATCH,
                ["geometric-search", "gm"],
                "Find geometrically similar assets in a catalog",
            )
            .after_help(crate::commands::examples(&[
                (
                    "Catalog parts that match a bracket 90% or better",
                    "pcli2 catalog geometric-match --path /Home/Parts/bracket.stl --threshold 90",
                ),
                (
                    "Name the catalog when you can search more than one",
                    "pcli2 catalog geometric-match --path /Home/Parts/bracket.stl --in catalog --format csv --headers",
                ),
            ]))
            .arg(threshold_parameter("Similarity threshold (0.00 to 100.00)")),
        )
        .subcommand(
            match_command(
                COMMAND_PART_MATCH,
                ["part-search", "pm"],
                "Find catalog assets that contain, or are contained in, an asset (part search)",
            )
            .arg(threshold_parameter("Similarity threshold (0.00 to 100.00)")),
        )
        .subcommand(
            match_command(
                COMMAND_VISUAL_MATCH,
                ["visual-search", "vm"],
                "Find visually similar assets in a catalog",
            )
            .arg(threshold_parameter(
                "Size threshold (0.00 to 100.00): filters matches by geometric size relative to the reference asset; higher is stricter, 0 disables size filtering",
            )),
        )
}

/// Execute catalog subcommands based on the provided arguments.
pub async fn execute_catalog_command(matches: &ArgMatches) -> Result<(), crate::error::CliError> {
    use crate::model::CatalogSearchKind;
    match matches.subcommand() {
        Some((COMMAND_LIST, sub_matches)) => {
            crate::actions::catalog::list_catalogs(sub_matches).await
        }
        Some((COMMAND_GEOMETRIC_MATCH, sub_matches)) => {
            crate::actions::catalog::catalog_match(sub_matches, CatalogSearchKind::Geometric).await
        }
        Some((COMMAND_PART_MATCH, sub_matches)) => {
            crate::actions::catalog::catalog_match(sub_matches, CatalogSearchKind::Part).await
        }
        Some((COMMAND_VISUAL_MATCH, sub_matches)) => {
            crate::actions::catalog::catalog_match(sub_matches, CatalogSearchKind::Visual).await
        }
        _ => Err(crate::error::CliError::UnsupportedSubcommand(
            matches
                .subcommand()
                .map(|(name, _)| name.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
        )),
    }
}
