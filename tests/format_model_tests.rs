/// Comprehensive tests for format utilities and model functions.
///
/// These tests provide regression protection for formatting and path normalization.
#[cfg(test)]
mod format_and_model_tests {
    use pcli2::format::{OutputFormat, OutputFormatOptions, FormattingError};
    use pcli2::format_utils::{FormatParams, FormatOptionsBuilder, FormatPreset};
    use pcli2::model::normalize_path;

    mod normalize_path_tests {
        use super::*;

        #[test]
        fn test_basic_path_normalization() {
            assert_eq!(normalize_path("/myroot/mysub/more/"), "/myroot/mysub/more");
            assert_eq!(normalize_path("myroot/mysub/more"), "/myroot/mysub/more");
            assert_eq!(normalize_path("/HOME"), "/");
            assert_eq!(normalize_path("////"), "/");
        }

        #[test]
        fn test_consecutive_slashes() {
            assert_eq!(normalize_path("/myroot//mysub///more/"), "/myroot/mysub/more");
            assert_eq!(normalize_path("Root//Folder"), "/Root/Folder");
            assert_eq!(normalize_path("//double//slash//test"), "/double/slash/test");
            assert_eq!(normalize_path("///"), "/");
            assert_eq!(normalize_path(""), "/");
        }

        #[test]
        fn test_home_handling_case_insensitive() {
            assert_eq!(normalize_path("/HOME"), "/");
            assert_eq!(normalize_path("/home"), "/");
            assert_eq!(normalize_path("/HOME/"), "/");
            assert_eq!(normalize_path("/home/"), "/");
            assert_eq!(normalize_path("/HOME/test"), "/test");
            assert_eq!(normalize_path("/home/test"), "/test");
            assert_eq!(normalize_path("/HOME/test/"), "/test");
            assert_eq!(normalize_path("/home/test/"), "/test");
            assert_eq!(normalize_path("/HoMe"), "/");
            assert_eq!(normalize_path("/hOmE/test"), "/test");
        }

        #[test]
        fn test_edge_cases() {
            assert_eq!(normalize_path("/"), "/");
            assert_eq!(normalize_path(""), "/");
            assert_eq!(normalize_path("   "), "/");
            assert_eq!(normalize_path("   /   "), "/");
            assert_eq!(normalize_path("   /test/   "), "/test");
            assert_eq!(normalize_path("test"), "/test");
            assert_eq!(normalize_path("test/"), "/test");
            assert_eq!(normalize_path("/test"), "/test");
            assert_eq!(normalize_path("/////test"), "/test");
            assert_eq!(normalize_path("test/////"), "/test");
        }

        #[test]
        fn test_trailing_slashes() {
            assert_eq!(normalize_path("/test/"), "/test");
            assert_eq!(normalize_path("/test//"), "/test");
            assert_eq!(normalize_path("/test///"), "/test");
            assert_eq!(normalize_path("test/"), "/test");
            assert_eq!(normalize_path("test//"), "/test");
            assert_eq!(normalize_path("test///"), "/test");
        }

        #[test]
        fn test_leading_slashes() {
            assert_eq!(normalize_path("//test"), "/test");
            assert_eq!(normalize_path("///test"), "/test");
            assert_eq!(normalize_path("////test"), "/test");
            assert_eq!(normalize_path("/////test"), "/test");
        }

        #[test]
        fn test_complex_paths() {
            assert_eq!(
                normalize_path("/HOME/Root/Folder//Subfolder///"),
                "/Root/Folder/Subfolder"
            );
            assert_eq!(
                normalize_path("//Root//Folder//Subfolder//"),
                "/Root/Folder/Subfolder"
            );
            assert_eq!(
                normalize_path("/Root/Folder/Subfolder/Asset.stl"),
                "/Root/Folder/Subfolder/Asset.stl"
            );
        }

        #[test]
        fn test_whitespace_handling() {
            assert_eq!(normalize_path("  /test  "), "/test");
            assert_eq!(normalize_path("\t/test\n"), "/test");
            assert_eq!(normalize_path("  /HOME/test  "), "/test");
        }

        #[test]
        fn test_home_prefix_variations() {
            // Test that /HOME prefix is removed regardless of case
            assert_eq!(normalize_path("/HOME/Folder"), "/Folder");
            assert_eq!(normalize_path("/home/Folder"), "/Folder");
            assert_eq!(normalize_path("/Home/Folder"), "/Folder");
            assert_eq!(normalize_path("/hOmE/Folder"), "/Folder");
            
            // Test with trailing slash
            assert_eq!(normalize_path("/HOME/Folder/"), "/Folder");
            assert_eq!(normalize_path("/home/Folder/"), "/Folder");
        }
    }

    mod output_format_tests {
        use super::*;

        #[test]
        fn test_format_names() {
            let names = OutputFormat::names();
            assert!(names.contains(&"json"));
            assert!(names.contains(&"csv"));
            assert!(names.contains(&"tree"));
            assert_eq!(names.len(), 3);
        }

        #[test]
        fn test_supported_formats() {
            let formats = OutputFormat::supported_formats();
            assert!(formats.contains(&"json"));
            assert!(formats.contains(&"csv"));
            assert!(formats.contains(&"tree"));
        }

        #[test]
        fn test_supports_format_case_insensitive() {
            assert!(OutputFormat::supports_format("json"));
            assert!(OutputFormat::supports_format("JSON"));
            assert!(OutputFormat::supports_format("Json"));
            assert!(OutputFormat::supports_format("csv"));
            assert!(OutputFormat::supports_format("CSV"));
            assert!(OutputFormat::supports_format("tree"));
            assert!(OutputFormat::supports_format("TREE"));
            assert!(!OutputFormat::supports_format("xml"));
            assert!(!OutputFormat::supports_format("yaml"));
        }

        #[test]
        fn test_from_string_with_options_json() {
            let options = OutputFormatOptions::default();
            let format = OutputFormat::from_string_with_options("json", options.clone()).unwrap();
            assert!(matches!(format, OutputFormat::Json(_)));
            
            let format = OutputFormat::from_string_with_options("JSON", options).unwrap();
            assert!(matches!(format, OutputFormat::Json(_)));
        }

        #[test]
        fn test_from_string_with_options_csv() {
            let options = OutputFormatOptions::default();
            let format = OutputFormat::from_string_with_options("csv", options.clone()).unwrap();
            assert!(matches!(format, OutputFormat::Csv(_)));
            
            let format = OutputFormat::from_string_with_options("CSV", options).unwrap();
            assert!(matches!(format, OutputFormat::Csv(_)));
        }

        #[test]
        fn test_from_string_with_options_tree() {
            let options = OutputFormatOptions::default();
            let format = OutputFormat::from_string_with_options("tree", options.clone()).unwrap();
            assert!(matches!(format, OutputFormat::Tree(_)));
            
            let format = OutputFormat::from_string_with_options("TREE", options).unwrap();
            assert!(matches!(format, OutputFormat::Tree(_)));
        }

        #[test]
        fn test_from_string_with_options_invalid() {
            let options = OutputFormatOptions::default();
            let result = OutputFormat::from_string_with_options("xml", options);
            assert!(result.is_err());
            
            match result.unwrap_err() {
                FormattingError::UnsupportedOutputFormat(fmt) => assert_eq!(fmt, "xml"),
                _ => panic!("Expected UnsupportedOutputFormat error"),
            }
        }

        #[test]
        fn test_from_string_with_options_safe() {
            let options = OutputFormatOptions::default();
            
            // Valid formats
            assert!(OutputFormat::from_string_with_options_safe("json", options.clone()).is_ok());
            assert!(OutputFormat::from_string_with_options_safe("csv", options.clone()).is_ok());
            assert!(OutputFormat::from_string_with_options_safe("tree", options.clone()).is_ok());
            
            // Invalid format
            assert!(OutputFormat::from_string_with_options_safe("xml", options).is_err());
        }

        #[test]
        fn test_from_string_with_options_safe_trimming() {
            let options = OutputFormatOptions::default();
            // Should handle whitespace
            assert!(OutputFormat::from_string_with_options_safe(" json ", options.clone()).is_ok());
            assert!(OutputFormat::from_string_with_options_safe(" csv ", options.clone()).is_ok());
            assert!(OutputFormat::from_string_with_options_safe(" tree ", options).is_ok());
        }

        #[test]
        fn test_default_format() {
            let default = OutputFormat::default();
            assert!(matches!(default, OutputFormat::Json(_)));
        }

        #[test]
        fn test_format_display() {
            let json_format = OutputFormat::Json(OutputFormatOptions::default());
            assert_eq!(format!("{}", json_format), "json");
            
            let csv_format = OutputFormat::Csv(OutputFormatOptions::default());
            assert_eq!(format!("{}", csv_format), "csv");
            
            let tree_format = OutputFormat::Tree(OutputFormatOptions::default());
            assert_eq!(format!("{}", tree_format), "tree");
        }

        #[test]
        fn test_from_str_trait() {
            let format: Result<OutputFormat, _> = "json".parse();
            assert!(format.is_ok());
            assert!(matches!(format.unwrap(), OutputFormat::Json(_)));
            
            let format: Result<OutputFormat, _> = "invalid".parse();
            assert!(format.is_err());
        }
    }

    mod output_format_options_tests {
        use super::*;

        #[test]
        fn test_default_options() {
            let options = OutputFormatOptions::default();
            assert!(!options.with_metadata);
            assert!(!options.with_headers);
            assert!(!options.pretty);
        }

        #[test]
        fn test_options_with_metadata() {
            let options = OutputFormatOptions {
                with_metadata: true,
                with_headers: false,
                pretty: false,
            };
            assert!(options.with_metadata);
            assert!(!options.with_headers);
            assert!(!options.pretty);
        }

        #[test]
        fn test_options_with_headers() {
            let options = OutputFormatOptions {
                with_metadata: false,
                with_headers: true,
                pretty: false,
            };
            assert!(!options.with_metadata);
            assert!(options.with_headers);
            assert!(!options.pretty);
        }

        #[test]
        fn test_options_pretty() {
            let options = OutputFormatOptions {
                with_metadata: false,
                with_headers: false,
                pretty: true,
            };
            assert!(!options.with_metadata);
            assert!(!options.with_headers);
            assert!(options.pretty);
        }

        #[test]
        fn test_options_clone() {
            let options = OutputFormatOptions {
                with_metadata: true,
                with_headers: true,
                pretty: true,
            };
            let cloned = options.clone();
            assert_eq!(options.with_metadata, cloned.with_metadata);
            assert_eq!(options.with_headers, cloned.with_headers);
            assert_eq!(options.pretty, cloned.pretty);
        }

        #[test]
        fn test_options_partial_eq() {
            let options1 = OutputFormatOptions {
                with_metadata: true,
                with_headers: false,
                pretty: true,
            };
            let options2 = OutputFormatOptions {
                with_metadata: true,
                with_headers: false,
                pretty: true,
            };
            assert_eq!(options1, options2);
        }
    }

    mod format_options_builder_tests {
        use super::*;

        #[test]
        fn test_builder_default() {
            let builder = FormatOptionsBuilder::new();
            let options = builder.build();
            assert!(!options.with_metadata);
            assert!(!options.with_headers);
            assert!(!options.pretty);
        }

        #[test]
        fn test_builder_with_metadata() {
            let options = FormatOptionsBuilder::new()
                .with_metadata(true)
                .build();
            assert!(options.with_metadata);
        }

        #[test]
        fn test_builder_with_headers() {
            let options = FormatOptionsBuilder::new()
                .with_headers(true)
                .build();
            assert!(options.with_headers);
        }

        #[test]
        fn test_builder_pretty() {
            let options = FormatOptionsBuilder::new()
                .pretty(true)
                .build();
            assert!(options.pretty);
        }

        #[test]
        fn test_builder_chaining() {
            let options = FormatOptionsBuilder::new()
                .with_metadata(true)
                .with_headers(true)
                .pretty(true)
                .build();
            assert!(options.with_metadata);
            assert!(options.with_headers);
            assert!(options.pretty);
        }

        #[test]
        fn test_builder_default_trait() {
            let builder = FormatOptionsBuilder::default();
            let options = builder.build();
            assert!(!options.with_metadata);
            assert!(!options.with_headers);
            assert!(!options.pretty);
        }
    }

    mod format_preset_tests {
        use super::*;

        #[test]
        fn test_human_readable_preset() {
            let preset = FormatPreset::HumanReadable;
            let format = preset.to_format("json");
            // Human readable should have pretty printing
            assert!(matches!(format, OutputFormat::Json(opts) if opts.pretty));
        }

        #[test]
        fn test_machine_readable_preset() {
            let preset = FormatPreset::MachineReadable;
            let format = preset.to_format("json");
            // Machine readable should not have pretty printing
            assert!(matches!(format, OutputFormat::Json(opts) if !opts.pretty));
        }

        #[test]
        fn test_verbose_preset() {
            let preset = FormatPreset::Verbose;
            let format = preset.to_format("json");
            // Verbose should have metadata and headers
            assert!(matches!(format, OutputFormat::Json(opts) if opts.with_metadata && opts.with_headers && opts.pretty));
        }

        #[test]
        fn test_compact_preset() {
            let preset = FormatPreset::Compact;
            let format = preset.to_format("json");
            // Compact should have no extra options
            assert!(matches!(format, OutputFormat::Json(opts) if !opts.with_metadata && !opts.with_headers && !opts.pretty));
        }

        #[test]
        fn test_tabular_preset() {
            let preset = FormatPreset::Tabular;
            let format = preset.to_format("csv");
            // Tabular should have headers
            assert!(matches!(format, OutputFormat::Csv(opts) if opts.with_headers));
        }

        #[test]
        fn test_preset_apply_to_json() {
            let preset = FormatPreset::HumanReadable;
            let original = OutputFormat::Json(OutputFormatOptions::default());
            let result = preset.apply_to(original);
            assert!(matches!(result, OutputFormat::Json(opts) if opts.pretty));
        }

        #[test]
        fn test_preset_apply_to_csv() {
            let preset = FormatPreset::Tabular;
            let original = OutputFormat::Csv(OutputFormatOptions::default());
            let result = preset.apply_to(original);
            assert!(matches!(result, OutputFormat::Csv(opts) if opts.with_headers));
        }

        #[test]
        fn test_preset_apply_to_tree() {
            let preset = FormatPreset::Verbose;
            let original = OutputFormat::Tree(OutputFormatOptions::default());
            let result = preset.apply_to(original);
            assert!(matches!(result, OutputFormat::Tree(opts) if opts.with_metadata));
        }
    }

    mod format_params_tests {
        use super::*;
        use clap::{Arg, ArgAction, Command};

        #[test]
        fn test_format_params_from_args() {
            let cmd = Command::new("test")
                .arg(Arg::new("format").long("format").action(ArgAction::Set))
                .arg(Arg::new("headers").long("headers").action(ArgAction::SetTrue))
                .arg(Arg::new("pretty").long("pretty").action(ArgAction::SetTrue))
                .arg(Arg::new("metadata").long("metadata").action(ArgAction::SetTrue));

            let matches = cmd.get_matches_from(vec![
                "test",
                "--format", "json",
                "--headers",
                "--pretty",
                "--metadata",
            ]);

            let params = FormatParams::from_args(&matches);
            
            assert!(matches!(params.format, OutputFormat::Json(_)));
            assert!(params.format_options.with_headers);
            assert!(params.format_options.pretty);
            assert!(params.format_options.with_metadata);
            assert_eq!(params.format_str, "json");
        }

        #[test]
        fn test_format_params_default_format() {
            let cmd = Command::new("test")
                .arg(Arg::new("format").long("format").action(ArgAction::Set))
                .arg(Arg::new("headers").long("headers").action(ArgAction::SetTrue))
                .arg(Arg::new("pretty").long("pretty").action(ArgAction::SetTrue))
                .arg(Arg::new("metadata").long("metadata").action(ArgAction::SetTrue));

            let matches = cmd.get_matches_from(vec!["test"]);

            let params = FormatParams::from_args(&matches);
            
            // Should default to JSON
            assert!(matches!(params.format, OutputFormat::Json(_)));
            assert_eq!(params.format_str, "json");
        }

        #[test]
        fn test_format_params_with_default() {
            let cmd = Command::new("test")
                .arg(Arg::new("format").long("format").action(ArgAction::Set))
                .arg(Arg::new("headers").long("headers").action(ArgAction::SetTrue))
                .arg(Arg::new("pretty").long("pretty").action(ArgAction::SetTrue))
                .arg(Arg::new("metadata").long("metadata").action(ArgAction::SetTrue));

            let matches = cmd.get_matches_from(vec!["test"]);

            let params = FormatParams::from_args_with_default(&matches, "csv");
            
            assert!(matches!(params.format, OutputFormat::Csv(_)));
            assert_eq!(params.format_str, "csv");
        }
    }
}
