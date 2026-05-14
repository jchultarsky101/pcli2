use crate::{
    configuration::Configuration,
    error::CliError,
    format::Formattable,
    model::AssetHealthReport,
    param_utils::{get_format_parameter_value, get_tenant},
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use tracing::trace;

pub async fn count_assets(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Counting assets...");

    let format = get_format_parameter_value(sub_matches).await;
    let configuration = Configuration::load_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;

    let assets = api.list_all_tenant_assets(&tenant.uuid).await?;
    let report = AssetHealthReport::from_assets(&assets);

    println!("{}", report.format(&format)?);

    Ok(())
}
