//! `pcli2 api`: a passthrough to any endpoint of the Physna API.
//!
//! pcli2 wraps some of the API's endpoints in commands; this reaches the others
//! without waiting for a release, with the same login, token renewal, retries and
//! tenant resolution the commands use.

use crate::commands::params::PARAMETER_INPUT;
use crate::error::CliError;
use crate::physna_v3::{PhysnaApiClient, TryDefault};
use clap::ArgMatches;

pub async fn run(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let raw_path = sub_matches
        .get_one::<String>("path")
        .ok_or_else(|| CliError::MissingRequiredArgument("path".to_string()))?;
    let body = request_body(sub_matches)?;
    let method = match sub_matches.get_one::<String>("method") {
        Some(method) => method
            .to_uppercase()
            .parse::<reqwest::Method>()
            .map_err(|e| CliError::MissingRequiredArgument(e.to_string()))?,
        None if body.is_some() => reqwest::Method::POST,
        None => reqwest::Method::GET,
    };

    let mut api = PhysnaApiClient::try_default()?;
    let path = if raw_path.contains("{tenantId}") {
        let configuration = crate::configuration::Configuration::load_or_create_default()?;
        let tenant = crate::param_utils::get_tenant(&mut api, sub_matches, &configuration).await?;
        raw_path.replace("{tenantId}", &tenant.uuid.to_string())
    } else {
        raw_path.clone()
    };

    let text = if sub_matches.get_flag("paginate") && method == reqwest::Method::GET {
        fetch_all_pages(&mut api, &path).await?
    } else {
        let (_, text) = api
            .raw_request(method, &path, body.as_ref())
            .await
            .map_err(CliError::PhysnaExtendedApiError)?;
        text
    };
    print_body(&text);
    Ok(())
}

/// The JSON body from `--input`, or from `-F`/`-f` fields; `None` without either.
fn request_body(sub_matches: &ArgMatches) -> Result<Option<serde_json::Value>, CliError> {
    if let Some(input) = sub_matches.get_one::<String>(PARAMETER_INPUT) {
        let text = if input == "-" {
            std::io::read_to_string(std::io::stdin())?
        } else {
            std::fs::read_to_string(input)?
        };
        let value = serde_json::from_str(&text).map_err(CliError::JsonError)?;
        return Ok(Some(value));
    }
    let mut object = serde_json::Map::new();
    for (id, typed) in [("field", true), ("raw-field", false)] {
        for field in sub_matches.get_many::<String>(id).into_iter().flatten() {
            let (key, value) = field.split_once('=').ok_or_else(|| {
                CliError::MissingRequiredArgument(format!("'{}' is not KEY=VALUE", field))
            })?;
            let value = if typed {
                serde_json::from_str(value)
                    .unwrap_or_else(|_| serde_json::Value::String(value.to_string()))
            } else {
                serde_json::Value::String(value.to_string())
            };
            object.insert(key.to_string(), value);
        }
    }
    Ok((!object.is_empty()).then_some(serde_json::Value::Object(object)))
}

/// Every page of a GET listing, merged: the first page's object with its one
/// list field holding every page's items, and without `pageData`.
async fn fetch_all_pages(api: &mut PhysnaApiClient, path: &str) -> Result<String, CliError> {
    let (base, query) = match path.split_once('?') {
        Some((base, query)) => (base, query.to_string()),
        None => (path, String::new()),
    };
    let other_params: Vec<&str> = query
        .split('&')
        .filter(|p| !p.is_empty() && !p.starts_with("page="))
        .collect();

    let mut pager = crate::paging::Pager::new("API listing");
    let mut merged: Option<serde_json::Value> = None;
    let mut list_key: Option<String> = None;
    let mut collected = 0;
    loop {
        let mut params = other_params.clone();
        let page_param = format!("page={}", pager.page());
        params.push(&page_param);
        let (_, text) = api
            .raw_request(
                reqwest::Method::GET,
                &format!("{}?{}", base, params.join("&")),
                None,
            )
            .await
            .map_err(CliError::PhysnaExtendedApiError)?;
        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            // Not JSON: nothing to page through.
            Err(_) => return Ok(text),
        };
        let page_data = value.get("pageData").cloned();
        let key = list_key.clone().or_else(|| {
            let lists: Vec<&String> = value
                .as_object()
                .map(|o| {
                    o.iter()
                        .filter(|(_, v)| v.is_array())
                        .map(|(k, _)| k)
                        .collect()
                })
                .unwrap_or_default();
            (lists.len() == 1).then(|| lists[0].clone())
        });
        let (Some(key), Some(page_data)) = (key, page_data) else {
            // Not a paged listing: print it as it came.
            return Ok(text);
        };
        list_key = Some(key.clone());
        let items = value
            .get(&key)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        collected += items.len();
        match merged.as_mut() {
            None => {
                let mut first = value.clone();
                if let Some(object) = first.as_object_mut() {
                    object.remove("pageData");
                }
                merged = Some(first);
            }
            Some(all) => {
                if let Some(list) = all.get_mut(&key).and_then(|v| v.as_array_mut()) {
                    list.extend(items);
                }
            }
        }
        let current = page_data
            .get("currentPage")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        let last = page_data
            .get("lastPage")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        if !pager.advance(current, last, collected) {
            break;
        }
    }
    Ok(merged.map(|v| v.to_string()).unwrap_or_default())
}

/// The response body on stdout: JSON pretty-printed for a person at a terminal,
/// passed through as it came otherwise.
fn print_body(text: &str) {
    use std::io::IsTerminal;
    if text.is_empty() {
        return;
    }
    if std::io::stdout().is_terminal() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
            if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                println!("{}", pretty);
                return;
            }
        }
    }
    println!("{}", text.trim_end());
}
