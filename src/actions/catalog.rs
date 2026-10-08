//! The `catalog` command group: list the catalogs you can search, and match
//! an asset against one.
//!
//! What counts as a catalog is read fresh from `/users/me` on every run (the
//! tenant cache is not trusted for it): a tenant Physna flags as a remote
//! search tenant, where the user is enabled. `--in` accepts nothing else. The
//! server would run a cross-tenant search between any two tenants the user
//! belongs to; pcli2 refuses to, so a search from one company's tenant is
//! never answered with another company's data, nor production with staging.

use crate::{
    commands::catalog::PARAMETER_IN,
    commands::params::{PARAMETER_LIMIT, PARAMETER_PATH, PARAMETER_UUID},
    error::CliError,
    format::OutputFormatter,
    model::{Catalog, CatalogList, CatalogMatchReport, CatalogSearchKind},
    physna_v3::{PhysnaApiClient, TryDefault},
};
use clap::ArgMatches;
use tracing::trace;
use uuid::Uuid;

const NOT_ENABLED: &str = "Catalog search is not enabled for your account: none of your tenants \
is a catalog you can search. Ask Physna to enable it.";

/// The catalogs the signed-in user can search, fresh from the server.
async fn fetch_catalogs(api: &mut PhysnaApiClient) -> Result<Vec<Catalog>, CliError> {
    let user = api.get_current_user().await?;
    Ok(Catalog::from_tenant_settings(&user.user.settings))
}

/// The catalog `--in` names, or the only one when it names none.
///
/// A name or ID that is not one of `catalogs` is refused the same way whether
/// or not it is another tenant of the user's.
pub fn choose_catalog(catalogs: &[Catalog], requested: Option<&str>) -> Result<Catalog, CliError> {
    if catalogs.is_empty() {
        return Err(CliError::FeatureUnavailable(NOT_ENABLED.to_string()));
    }
    let names = || {
        catalogs
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    match requested.map(str::trim) {
        Some(requested) => {
            let by_id = Uuid::parse_str(requested).ok();
            catalogs
                .iter()
                .find(|c| Some(c.id) == by_id || c.name.eq_ignore_ascii_case(requested))
                .cloned()
                .ok_or_else(|| {
                    CliError::InvalidArgument(format!(
                        "'{}' is not a catalog you can search. Your catalogs: {} (see 'pcli2 catalog list').",
                        requested,
                        names()
                    ))
                })
        }
        None if catalogs.len() == 1 => Ok(catalogs[0].clone()),
        None => Err(CliError::InvalidArgument(format!(
            "You can search more than one catalog; name one with --in: {}.",
            names()
        ))),
    }
}

/// `catalog list`.
pub async fn list_catalogs(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"catalog list\" command...");
    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;
    let mut api = PhysnaApiClient::try_default()?;
    let catalogs = fetch_catalogs(&mut api).await?;
    if catalogs.is_empty() {
        crate::error_utils::report_warning(&NOT_ENABLED);
    }
    crate::format::print_output(&CatalogList { catalogs }.format(format)?);
    Ok(())
}

/// `catalog geometric-match | part-match | visual-match`.
pub async fn catalog_match(
    sub_matches: &ArgMatches,
    kind: CatalogSearchKind,
) -> Result<(), CliError> {
    trace!("Executing catalog {:?} match command...", kind);
    let format = crate::format_utils::FormatParams::from_args(sub_matches).format;
    let threshold = crate::actions::utils::threshold_from_args(sub_matches);
    let limit = sub_matches
        .get_one::<usize>(PARAMETER_LIMIT)
        .copied()
        .unwrap_or(100);

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let tenant_uuid = *ctx.tenant_uuid();
    let tenant_name = ctx.tenant().name.clone();

    // The catalog is settled before anything is read from the asset's tenant,
    // so a refused --in costs no more than the /users/me call.
    let catalogs = fetch_catalogs(ctx.api()).await?;
    let catalog = choose_catalog(
        &catalogs,
        sub_matches
            .get_one::<String>(PARAMETER_IN)
            .map(String::as_str),
    )?;
    if catalog.id == tenant_uuid {
        return Err(CliError::InvalidArgument(format!(
            "The asset is in the catalog '{}' itself; search within a tenant with 'pcli2 asset {}-match'.",
            catalog.name,
            match kind {
                CatalogSearchKind::Geometric => "geometric",
                CatalogSearchKind::Part => "part",
                CatalogSearchKind::Visual => "visual",
            }
        )));
    }

    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        sub_matches.get_one::<Uuid>(PARAMETER_UUID),
        sub_matches.get_one::<String>(PARAMETER_PATH),
    )
    .await?;
    let asset_uuid = asset.uuid();

    let progress = crate::terminal::spinner(&format!("Searching catalog '{}'...", catalog.name));
    let result = ctx
        .api()
        .catalog_search(
            kind,
            &tenant_uuid,
            &asset_uuid,
            &catalog.id,
            threshold,
            limit,
        )
        .await;
    progress.finish_and_clear();
    let (mut matches, more) = result?;

    // Belt and braces: whatever the server sends, only the catalog's own
    // assets are shown.
    let before = matches.len();
    matches.retain(|m| m.asset.tenant_id == catalog.id);
    if matches.len() < before {
        crate::error_utils::report_warning(&format!(
            "Dropped {} match(es) the server returned from outside catalog '{}'.",
            before - matches.len(),
            catalog.name
        ));
    }
    if more {
        crate::error_utils::report_warning(&format!(
            "Showing the first {} matches; raise --limit to see more.",
            limit
        ));
    }

    let ui_base_url = ctx.configuration().get_ui_base_url();
    for m in &mut matches {
        let comparison = match kind {
            CatalogSearchKind::Geometric => crate::ui_url::Comparison::Geometric {
                match_percentage: m.match_percentage.unwrap_or(0.0),
            },
            CatalogSearchKind::Part => crate::ui_url::Comparison::Part {
                forward: m.forward_match_percentage.unwrap_or(0.0),
                reverse: m.reverse_match_percentage.unwrap_or(0.0),
            },
            CatalogSearchKind::Visual => crate::ui_url::Comparison::Visual,
        };
        m.comparison_url = Some(crate::ui_url::cross_tenant_compare_url(
            &ui_base_url,
            &tenant_name,
            &tenant_uuid,
            &asset_uuid,
            &catalog.id,
            &m.asset.uuid,
            comparison,
        ));
    }

    let report = CatalogMatchReport {
        search_type: kind,
        reference_asset: crate::actions::assets::match_ops::reference_asset_response(
            &asset,
            asset_uuid,
            tenant_uuid,
        ),
        catalog,
        matches,
    };
    crate::format::print_output(&report.format(format)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(name: &str, id: &str) -> Catalog {
        Catalog {
            id: Uuid::parse_str(id).unwrap(),
            name: name.to_string(),
            description: name.to_uppercase(),
            role: "search".to_string(),
        }
    }

    const A: &str = "a0d94427-d7eb-4b28-b624-49c68ce83686";
    const B: &str = "11111111-2222-3333-4444-555555555555";

    #[test]
    fn no_catalog_means_the_feature_is_not_enabled() {
        let err = choose_catalog(&[], None).unwrap_err();
        assert!(matches!(err, CliError::FeatureUnavailable(_)));
        // Naming a tenant does not get around it.
        let err = choose_catalog(&[], Some("demo-2")).unwrap_err();
        assert!(matches!(err, CliError::FeatureUnavailable(_)));
    }

    #[test]
    fn the_only_catalog_is_the_default() {
        let only = [catalog("catalog", A)];
        assert_eq!(choose_catalog(&only, None).unwrap().name, "catalog");
    }

    #[test]
    fn with_several_catalogs_one_must_be_named() {
        let two = [catalog("catalog", A), catalog("parts", B)];
        let err = choose_catalog(&two, None).unwrap_err();
        assert!(matches!(err, CliError::InvalidArgument(ref m) if m.contains("catalog, parts")));
        assert_eq!(choose_catalog(&two, Some("PARTS")).unwrap().name, "parts");
        assert_eq!(choose_catalog(&two, Some(A)).unwrap().name, "catalog");
    }

    #[test]
    fn a_tenant_that_is_not_a_catalog_is_refused() {
        let only = [catalog("catalog", A)];
        for other in ["demo-2", "68555ebf-f09c-4861-96b1-692d2ec10de7", ""] {
            let err = choose_catalog(&only, Some(other)).unwrap_err();
            assert!(
                matches!(err, CliError::InvalidArgument(ref m) if m.contains("not a catalog you can search")),
                "{other:?} must be refused"
            );
        }
    }

    #[test]
    fn only_flagged_enabled_tenants_are_catalogs() {
        use crate::model::TenantSetting;
        let setting = |name: &str, id: &str, remote: bool, enabled: bool| TenantSetting {
            tenant_uuid: Uuid::parse_str(id).unwrap(),
            tenant_role: "search".to_string(),
            user_enabled: enabled,
            tenant_display_name: name.to_string(),
            tenant_short_name: name.to_string(),
            is_remote_search_tenant: remote,
            asset_limit: None,
            asset_limit_warning_threshold_percent: None,
        };
        let catalogs = Catalog::from_tenant_settings(&[
            setting("zeta", A, true, true),
            setting(
                "demo-1",
                "68555ebf-f09c-4861-96b1-692d2ec10de7",
                false,
                true,
            ),
            setting(
                "disabled",
                "22222222-2222-3333-4444-555555555555",
                true,
                false,
            ),
            setting("alpha", B, true, true),
        ]);
        let names: Vec<&str> = catalogs.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["alpha", "zeta"]);
    }
}
