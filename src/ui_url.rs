//! Links into the Physna web application.
//!
//! The comparison and asset links in match reports were built by hand in more than
//! a dozen places, each deciding for itself whether the configured UI base URL
//! already ends in `/tenants`. One copy (text match) did not, and produced
//! `.../tenants/tenants/...`. They are built here, once.

use uuid::Uuid;

/// Which comparison view a link opens, with the scores it shows.
#[derive(Debug, Clone, Copy)]
pub enum Comparison {
    Geometric { match_percentage: f64 },
    Part { forward: f64, reverse: f64 },
    Visual,
}

/// `<ui base>/tenants/<tenant short name>`, whether or not the configured base URL
/// already ends in `/tenants`.
pub fn tenant_base(ui_base_url: &str, tenant_name: &str) -> String {
    let base = ui_base_url.trim_end_matches('/');
    if base.ends_with("/tenants") {
        format!("{}/{}", base, tenant_name)
    } else {
        format!("{}/tenants/{}", base, tenant_name)
    }
}

/// The side-by-side comparison of two assets of one tenant.
pub fn compare_url(
    ui_base_url: &str,
    tenant_name: &str,
    tenant_uuid: &Uuid,
    reference: &Uuid,
    candidate: &Uuid,
    comparison: Comparison,
) -> String {
    format!(
        "{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&{}",
        tenant_base(ui_base_url, tenant_name),
        reference,
        candidate,
        tenant_uuid,
        tenant_uuid,
        scores(comparison)
    )
}

/// The side-by-side comparison of an asset with one in another tenant (a
/// catalog), opened from the reference asset's tenant.
pub fn cross_tenant_compare_url(
    ui_base_url: &str,
    tenant_name: &str,
    tenant_uuid: &Uuid,
    reference: &Uuid,
    candidate_tenant_uuid: &Uuid,
    candidate: &Uuid,
    comparison: Comparison,
) -> String {
    format!(
        "{}/compare?asset1Id={}&asset2Id={}&tenant1Id={}&tenant2Id={}&{}",
        tenant_base(ui_base_url, tenant_name),
        reference,
        candidate,
        tenant_uuid,
        candidate_tenant_uuid,
        scores(comparison)
    )
}

/// The query parameters naming the comparison view and its scores.
fn scores(comparison: Comparison) -> String {
    match comparison {
        Comparison::Geometric { match_percentage } => {
            format!(
                "searchType=geometric&matchPercentage={:.2}",
                match_percentage
            )
        }
        Comparison::Part { forward, reverse } => format!(
            "searchType=part&forwardMatch={:.2}&reverseMatch={:.2}",
            forward, reverse
        ),
        Comparison::Visual => "searchType=visual".to_string(),
    }
}

/// One asset's page.
pub fn asset_url(ui_base_url: &str, tenant_name: &str, asset: &Uuid) -> String {
    format!("{}/asset/{}", tenant_base(ui_base_url, tenant_name), asset)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &str = "22222222-2222-2222-2222-222222222222";

    #[test]
    fn a_cross_tenant_link_names_both_tenants() {
        let ours = Uuid::parse_str(T).unwrap();
        let catalog = Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap();
        let (a, b) = (Uuid::nil(), Uuid::max());
        assert_eq!(
            cross_tenant_compare_url("https://app.physna.com/tenants", "acme", &ours, &a, &catalog, &b, Comparison::Visual),
            format!("https://app.physna.com/tenants/acme/compare?asset1Id={a}&asset2Id={b}&tenant1Id={T}&tenant2Id={catalog}&searchType=visual")
        );
    }

    #[test]
    fn a_base_url_with_or_without_tenants_gives_the_same_link() {
        let tenant = Uuid::parse_str(T).unwrap();
        let a = Uuid::nil();
        for base in [
            "https://app.physna.com",
            "https://app.physna.com/",
            "https://app.physna.com/tenants",
            "https://app.physna.com/tenants/",
        ] {
            assert_eq!(
                asset_url(base, "acme", &a),
                "https://app.physna.com/tenants/acme/asset/00000000-0000-0000-0000-000000000000"
            );
            assert_eq!(
                compare_url(base, "acme", &tenant, &a, &a, Comparison::Geometric { match_percentage: 97.456 }),
                format!("https://app.physna.com/tenants/acme/compare?asset1Id={a}&asset2Id={a}&tenant1Id={T}&tenant2Id={T}&searchType=geometric&matchPercentage=97.46")
            );
        }
        assert!(compare_url(
            "https://x",
            "acme",
            &tenant,
            &a,
            &a,
            Comparison::Part {
                forward: 1.0,
                reverse: 2.0
            }
        )
        .ends_with("&searchType=part&forwardMatch=1.00&reverseMatch=2.00"));
        assert!(
            compare_url("https://x", "acme", &tenant, &a, &a, Comparison::Visual)
                .ends_with("&searchType=visual")
        );
    }
}
