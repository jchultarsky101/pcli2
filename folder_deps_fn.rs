/// Execute the folder dependencies command to get dependencies for all assembly assets in one or more folders
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
        return Err(CliError::MissingRequiredArgument("At least one folder path must be provided".to_string()));
    }

    let tenant_uuid = *ctx.tenant_uuid();
    
    // Collect all dependencies from all specified folders
    let mut all_dependencies = Vec::new();
    let mut all_assembly_trees = Vec::new();

    for folder_path in &folder_paths {
        // List all assets in the folder
        let assets_response = ctx.api().list_assets_by_parent_folder_path(&tenant_uuid, folder_path).await?;
        
        // Process each asset in the folder that is an assembly (has dependencies)
        for asset in assets_response.assets() {
            // Only process assemblies (assets that have dependencies)
            if asset.is_assembly() {
                trace!("Processing assembly: {} (path: {})", asset.name(), asset.path());
                
                // Get the full assembly tree with all recursive dependencies for this asset
                let assembly_tree = ctx.api().get_asset_dependencies_by_path(&tenant_uuid, asset.path().as_str()).await?;
                
                // For tree and JSON formats, we'll collect the assembly trees to preserve hierarchy
                if matches!(format, crate::format::OutputFormat::Tree(_) | crate::format::OutputFormat::Json(_)) {
                    all_assembly_trees.push(assembly_tree);
                } else {
                    // For other formats (CSV), extract all dependencies from the tree structure
                    let asset_dependencies = extract_all_dependencies_from_tree(&assembly_tree);
                    
                    // Add all dependencies from this asset's tree to the combined list
                    all_dependencies.extend(asset_dependencies);
                }
            }
        }
    }

    // Output the results based on the requested format
    if matches!(format, crate::format::OutputFormat::Tree(_) | crate::format::OutputFormat::Json(_)) {
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
        let dependency_list = crate::model::AssetDependencyList {
            path: format!("Combined dependencies from folders: {}", folder_paths.join(", ")),
            dependencies: all_dependencies,
        };

        println!("{}", dependency_list.format(format)?);
    }

    Ok(())
}