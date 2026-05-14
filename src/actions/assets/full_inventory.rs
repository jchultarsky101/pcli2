use crate::{
    configuration::Configuration,
    error::CliError,
    format::{Formattable, OutputFormatter},
    model::{AssetHealthReport, AssetList},
    param_utils::{get_format_parameter_value, get_tenant},
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use tracing::trace;

async fn fetch_all_assets(sub_matches: &ArgMatches) -> Result<AssetList, CliError> {
    let configuration = Configuration::load_default()?;
    let mut api = PhysnaApiClient::try_default()?;
    let tenant = get_tenant(&mut api, sub_matches, &configuration).await?;
    let assets = api.list_all_tenant_assets(&tenant.uuid).await?;
    Ok(assets)
}

pub async fn inventory(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Listing full asset inventory...");
    let format = get_format_parameter_value(sub_matches).await;
    let assets = fetch_all_assets(sub_matches).await?;
    println!("{}", assets.format(format)?);
    Ok(())
}

pub async fn count_assets(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Counting assets...");
    let format = get_format_parameter_value(sub_matches).await;
    let assets = fetch_all_assets(sub_matches).await?;
    let report = AssetHealthReport::from_assets(&assets);
    println!("{}", report.format(&format)?);
    Ok(())
}
