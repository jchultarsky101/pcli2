//! Print asset functionality.
//!
//! This module provides functionality for printing asset information,
//! dependencies, and metadata.

use crate::{
    commands::params::{PARAMETER_PATH, PARAMETER_UUID},
    error::CliError,
    format::OutputFormatter,
    param_utils::get_format_parameter_value,
};
use clap::ArgMatches;
use tracing::trace;
use uuid::Uuid;

/// Print information about a specific asset.
///
/// This function handles the "asset get" command, retrieving and displaying
/// information about a specific asset identified by either its UUID or path.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset information was printed successfully
/// * `Err(CliError)` - If an error occurred during the retrieval
pub async fn print_asset(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset get\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;

    // Use FormatParams for consistent format parameter handling
    let format_params = crate::format_utils::FormatParams::from_args(sub_matches);
    let format = format_params.format;
    let with_metadata = format_params.format_options.with_metadata;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Format the asset considering the metadata flag
    println!(
        "{}",
        asset.format_with_metadata_flag(format, with_metadata)?
    );

    Ok(())
}

/// Print the dependencies of a specific asset.
///
/// This function handles the "asset dependencies" command, retrieving and displaying
/// the dependency tree for a specific asset identified by either its UUID or path.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset dependencies were printed successfully
/// * `Err(CliError)` - If an error occurred during the retrieval
pub async fn print_asset_dependencies(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"asset dependencies\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let format = get_format_parameter_value(sub_matches).await;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    // Get the full assembly tree with all recursive dependencies
    let assembly_tree = ctx
        .api()
        .get_asset_dependencies_by_path(&tenant_uuid, asset.path().as_str())
        .await?;

    // For tree and JSON formats, output the assembly tree directly to preserve hierarchy
    if matches!(format, crate::format::OutputFormat::Tree(_))
        || matches!(format, crate::format::OutputFormat::Json(_))
    {
        println!("{}", assembly_tree.format(format)?);
    } else {
        // For other formats (CSV), extract all dependencies from the full tree structure
        let all_dependencies = extract_all_dependencies_from_tree(&assembly_tree);

        // Create an AssetDependencyList from the response to format properly
        let dependency_list = crate::model::AssetDependencyList {
            path: asset.path().to_string(),
            dependencies: all_dependencies,
        };

        println!("{}", dependency_list.format(format)?);
    }

    Ok(())
}

/// Print the metadata of a specific asset.
///
/// This function handles the "asset metadata get" command, retrieving and displaying
/// metadata for a specific asset identified by either its UUID or path.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the asset metadata was printed successfully
/// * `Err(CliError)` - If an error occurred during the retrieval
pub async fn print_asset_metadata(sub_matches: &ArgMatches) -> Result<(), CliError> {
    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let format = get_format_parameter_value(sub_matches).await;

    let asset_uuid_param = sub_matches.get_one::<Uuid>(PARAMETER_UUID);
    let asset_path_param = sub_matches.get_one::<String>(PARAMETER_PATH);

    // Extract tenant UUID before calling resolve_asset to avoid borrowing conflicts
    let tenant_uuid = *ctx.tenant_uuid();

    // Resolve asset ID from either UUID parameter or path using the helper function
    let asset = crate::actions::utils::resolve_asset(
        ctx.api(),
        &tenant_uuid,
        asset_uuid_param,
        asset_path_param,
    )
    .await?;

    if let Some(metadata) = asset.metadata() {
        println!("{}", metadata.format(format)?);
    }

    Ok(())
}

/// Execute the folder dependencies command to get dependencies for all assembly assets in one or more folders
///
/// This function handles the "folder dependencies" command, retrieving and displaying
/// dependencies for all assembly assets in the specified folders.
///
/// # Arguments
///
/// * `sub_matches` - The command-line argument matches containing the command parameters
///
/// # Returns
///
/// * `Ok(())` - If the folder dependencies were printed successfully
/// * `Err(CliError)` - If an error occurred during the retrieval
pub async fn print_folder_dependencies(sub_matches: &ArgMatches) -> Result<(), CliError> {
    trace!("Executing \"folder dependencies\" command...");

    let mut ctx = crate::context::ExecutionContext::from_args(sub_matches).await?;
    let format = get_format_parameter_value(sub_matches).await;

    // Get folder paths from the command line arguments
    let folder_paths: Vec<String> = sub_matches
        .get_many::<String>(crate::commands::params::PARAMETER_FOLDER_PATH)
        .unwrap_or_default()
        .map(|s| s.to_string())
        .collect();

    if folder_paths.is_empty() {
        return Err(CliError::MissingRequiredArgument(
            "At least one folder path must be provided".to_string(),
        ));
    }

    let tenant_uuid = *ctx.tenant_uuid();

    // Check if progress should be displayed
    let show_progress = sub_matches.get_flag(crate::commands::params::PARAMETER_PROGRESS);

    // Create progress bars if requested
    let multi_progress = if show_progress {
        let mp = indicatif::MultiProgress::new();

        // Add an overall progress bar
        let overall_pb = mp.add(indicatif::ProgressBar::new(folder_paths.len() as u64));
        overall_pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) - Processing folders")
                .unwrap()
                .progress_chars("#>-")
        );
        Some((mp, overall_pb))
    } else {
        None
    };

    // Collect all dependencies from all specified folders
    let mut all_dependencies = Vec::new();
    let mut all_assembly_trees = Vec::new();

    for folder_path in folder_paths.iter() {
        // Update overall progress if enabled
        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.set_message(format!("Processing folder: {}", folder_path));
        }

        // List all assets in the folder
        let assets_response = ctx
            .api()
            .list_assets_by_parent_folder_path(&tenant_uuid, folder_path)
            .await?;

        // Count total assemblies in this folder for progress tracking
        let assemblies: Vec<_> = assets_response
            .get_all_assets()
            .into_iter()
            .filter(|asset| asset.is_assembly())
            .collect();

        // Create individual progress bar for this folder if progress is enabled
        let folder_progress = if let Some((ref mp, _)) = multi_progress {
            let pb = mp.add(indicatif::ProgressBar::new(assemblies.len() as u64));
            pb.set_style(
                indicatif::ProgressStyle::default_bar()
                    .template(&format!(
                        "{{spinner:.yellow}} Processing assets in {}: {{pos}}/{{len}} {{msg}}",
                        folder_path
                    ))
                    .unwrap()
                    .progress_chars("#>-"),
            );
            Some(pb)
        } else {
            None
        };

        // Process each asset in the folder that is an assembly (has dependencies)
        for asset in assemblies {
            if let Some(ref pb) = folder_progress {
                pb.set_message(format!("Getting dependencies for: {}", asset.name()));
            }

            trace!(
                "Processing assembly: {} (path: {})",
                asset.name(),
                asset.path()
            );

            // Get the full assembly tree with all recursive dependencies for this asset
            let assembly_tree = ctx
                .api()
                .get_asset_dependencies_by_path(&tenant_uuid, asset.path().as_str())
                .await?;

            // For tree and JSON formats, we'll collect the assembly trees to preserve hierarchy
            let format_is_tree = matches!(format, crate::format::OutputFormat::Tree(_));
            let format_is_json = matches!(format, crate::format::OutputFormat::Json(_));
            if format_is_tree || format_is_json {
                all_assembly_trees.push(assembly_tree);
            } else {
                // For other formats (CSV), extract all dependencies from the tree structure
                let mut asset_dependencies = extract_all_dependencies_from_tree(&assembly_tree);

                // Update each dependency to include the original asset path information (for ASSET_PATH column)
                // The assembly_path should remain as the relative path within the assembly hierarchy
                for dep in &mut asset_dependencies {
                    // The assembly_path should already contain the relative path within the assembly hierarchy
                    // from the extract_all_dependencies_from_tree function, so we don't modify it here
                    // It represents the path from the root of this assembly to the dependency

                    // Set the original asset path for proper CSV output
                    dep.original_asset_path = Some(asset.path().to_string());
                }

                // Add all dependencies from this asset's tree to the combined list
                all_dependencies.extend(asset_dependencies);
            }

            // Update folder progress if enabled
            if let Some(ref pb) = folder_progress {
                pb.inc(1);
            }
        }

        // Finish folder progress bar if enabled
        if let Some(pb) = folder_progress {
            pb.finish_and_clear();
        }

        // Update overall progress if enabled
        if let Some((_, ref overall_pb)) = multi_progress {
            overall_pb.inc(1);
        }
    }

    // Finish overall progress bar if enabled
    if let Some((_, ref overall_pb)) = multi_progress {
        overall_pb.finish_with_message(format!("Processed {} folders", folder_paths.len()));
    }

    // Output the results based on the requested format
    let format_is_tree = matches!(format, crate::format::OutputFormat::Tree(_));
    let format_is_json = matches!(format, crate::format::OutputFormat::Json(_));
    if format_is_tree || format_is_json {
        // For tree and JSON formats, if we have multiple assembly trees, we need to handle them appropriately
        if all_assembly_trees.len() == 1 {
            // If there's only one tree, just output it directly
            println!("{}", all_assembly_trees[0].format(format)?);
        } else if all_assembly_trees.is_empty() {
            // If no assembly trees were found, output an empty result
            if matches!(format, crate::format::OutputFormat::Json(_)) {
                println!("[]"); // Output empty array for JSON format
            } else {
                println!("No assembly assets found in the specified folders.");
            }
        } else {
            // If there are multiple trees, output them separately with separators
            for (i, tree) in all_assembly_trees.iter().enumerate() {
                if i > 0 {
                    println!("---"); // Separator between different folder results
                }
                println!("{}", tree.format(format.clone())?);
            }
        }
    } else {
        // For CSV format, create an AssetDependencyList with all collected dependencies
        // Use a more appropriate path that indicates this is from multiple assets in the specified folders
        let dependency_list = crate::model::AssetDependencyList {
            path: "MULTIPLE_ASSETS".to_string(), // Indicate this represents multiple assets
            dependencies: all_dependencies,
        };

        println!("{}", dependency_list.format(format)?);
    }

    Ok(())
}

// Helper function to extract all dependencies from AssemblyTree recursively
fn extract_all_dependencies_from_tree(
    assembly_tree: &crate::model::AssemblyTree,
) -> Vec<crate::model::AssetDependency> {
    let mut all_dependencies = Vec::new();

    // Process all nodes in the tree recursively, starting with the root assembly name as the parent path
    let root_name = assembly_tree.root().asset().name();
    collect_dependencies_recursive(assembly_tree.root(), &mut all_dependencies, root_name);

    all_dependencies
}

// Recursive helper to collect all dependencies with assembly path tracking
fn collect_dependencies_recursive(
    node: &crate::model::AssemblyNode,
    dependencies: &mut Vec<crate::model::AssetDependency>,
    parent_assembly_path: String,
) {
    for child in node.children() {
        // Calculate the assembly path for this child
        let child_name = child.asset().name();
        let current_assembly_path = if parent_assembly_path.is_empty() {
            child_name.clone()
        } else {
            format!("{}/{}", parent_assembly_path, child_name)
        };

        // Create an AssetResponse from the child asset
        let asset_response = crate::model::AssetResponse {
            uuid: child.asset().uuid(),
            tenant_id: Uuid::nil(), // Placeholder - would need actual tenant ID if available
            path: child.asset().path(),
            folder_id: None,
            asset_type: child
                .asset()
                .file_type()
                .cloned()
                .unwrap_or_else(|| "unknown".to_string()),
            created_at: child.asset().created_at().cloned().unwrap_or_default(),
            updated_at: child.asset().updated_at().cloned().unwrap_or_default(),
            state: child
                .asset()
                .processing_status()
                .cloned()
                .unwrap_or_else(|| "missing".to_string()),
            is_assembly: child.has_children(),
            metadata: std::collections::HashMap::new(), // Empty metadata
            parent_folder_id: None,
            owner_id: None,
        };

        // Create AssetDependency from the child
        let asset_dependency = crate::model::AssetDependency {
            path: child.asset().path(),
            asset: Some(asset_response),
            occurrences: 1, // Default occurrence count
            has_dependencies: child.has_children(),
            assembly_path: current_assembly_path.clone(), // Clone to use in both places
            original_asset_path: None, // This will be set when processing folder dependencies
        };

        dependencies.push(asset_dependency);

        // Recursively process children of this child with updated assembly path
        collect_dependencies_recursive(child, dependencies, current_assembly_path);
    }
}
