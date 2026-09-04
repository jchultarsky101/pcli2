/// Comprehensive tests for error types and conversions.
///
/// These tests ensure error handling behaves correctly and provide
/// regression protection during refactoring.
#[cfg(test)]
mod error_tests {
    use pcli2::configuration::ConfigurationError;
    use pcli2::error::CliError;
    use pcli2::format::FormattingError;
    use uuid::Uuid;

    mod cli_error {
        use super::*;

        #[test]
        fn test_unsupported_subcommand_error() {
            let error = CliError::UnsupportedSubcommand("test-command".to_string());
            let error_str = error.to_string();
            // The error message is "Undefined or unsupported subcommand" followed by the command
            assert!(
                error_str.contains("unsupported subcommand") || error_str.contains("test-command")
            );
            assert_eq!(error.exit_code().code(), 64); // UsageError
        }

        #[test]
        fn test_configuration_error_conversion() {
            let config_error = ConfigurationError::FailedToFindConfigurationDirectory;
            let cli_error: CliError = config_error.into();

            match cli_error {
                CliError::ConfigurationError(_) => (),
                _ => panic!("Expected ConfigurationError variant"),
            }
        }

        #[test]
        fn test_formatting_error_conversion() {
            let format_error = FormattingError::UnsupportedOutputFormat("invalid".to_string());
            let cli_error: CliError = format_error.into();

            match cli_error {
                CliError::FormattingError(_) => (),
                _ => panic!("Expected FormattingError variant"),
            }
        }

        #[test]
        fn test_security_error() {
            let error = CliError::SecurityError("test error".to_string());
            // SecurityError message is not included in Display, only in Debug
            assert_eq!(error.exit_code().code(), 100); // AuthError
        }

        #[test]
        fn test_missing_required_argument() {
            let error = CliError::MissingRequiredArgument("--file".to_string());
            let error_str = error.to_string();
            assert!(error_str.contains("--file"));
            assert_eq!(error.exit_code().code(), 64); // UsageError
        }

        #[test]
        fn test_tenant_not_found() {
            let error = CliError::TenantNotFound {
                identifier: "test-tenant".to_string(),
            };
            let error_str = error.to_string();
            assert!(error_str.contains("test-tenant"));
            assert_eq!(error.exit_code().code(), 67); // NotFound
        }

        #[test]
        fn test_folder_not_found() {
            let error = CliError::FolderNotFound("/Root/Test".to_string(), String::new());
            let error_str = error.to_string();
            assert!(error_str.contains("/Root/Test"));
            assert_eq!(error.exit_code().code(), 67); // NotFound
        }

        #[test]
        fn test_folder_rename_failed() {
            let error = CliError::FolderRenameFailed(
                "uuid-123".to_string(),
                "permission denied".to_string(),
            );
            let error_str = error.to_string();
            assert!(error_str.contains("uuid-123"));
            assert!(error_str.contains("permission denied"));
        }

        #[test]
        fn test_json_error_conversion() {
            let invalid_json = "not valid json";
            let result: Result<serde_json::Value, _> = serde_json::from_str(invalid_json);
            assert!(result.is_err());

            let json_error = result.unwrap_err();
            let cli_error: CliError = json_error.into();

            match cli_error {
                CliError::JsonError(_) => (),
                _ => panic!("Expected JsonError variant"),
            }
        }

        #[test]
        fn test_uuid_parsing_error_conversion() {
            let invalid_uuid = "not-a-uuid";
            let result: Result<Uuid, _> = invalid_uuid.parse();
            assert!(result.is_err());

            let uuid_error = result.unwrap_err();
            let cli_error: CliError = uuid_error.into();

            match cli_error {
                CliError::UuidParsingError(_) => (),
                _ => panic!("Expected UuidParsingError variant"),
            }
        }

        #[test]
        fn exit_codes_follow_the_documented_contract() {
            use pcli2::actions::CliActionError;
            use pcli2::physna_v3::ApiError;

            // Not found -> 67
            assert_eq!(
                CliError::FolderNotFound("test".to_string(), String::new())
                    .exit_code()
                    .code(),
                67
            );
            // Authentication -> 100, whether raised directly or inside an action
            assert_eq!(
                CliError::PhysnaExtendedApiError(ApiError::InvalidToken)
                    .exit_code()
                    .code(),
                100
            );
            assert_eq!(
                CliError::ActionError(CliActionError::ApiError(ApiError::AuthError("x".into())))
                    .exit_code()
                    .code(),
                100
            );
            // Server-side error status -> 102
            assert_eq!(
                CliError::PhysnaExtendedApiError(ApiError::HttpStatus {
                    status: 503,
                    message: "down".into()
                })
                .exit_code()
                .code(),
                102
            );
            // An incomplete report is a temporary failure -> 69
            assert_eq!(
                CliError::ActionError(CliActionError::IncompleteReport {
                    attempted: 10,
                    failed: 5
                })
                .exit_code()
                .code(),
                69
            );
            // A missing input file -> 66
            assert_eq!(
                CliError::ActionError(CliActionError::IoError(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "gone"
                )))
                .exit_code()
                .code(),
                66
            );
        }
    }

    mod action_error {
        use pcli2::actions::CliActionError;

        #[test]
        fn test_json_error() {
            let invalid_json = "invalid";
            let result: Result<serde_json::Value, _> = serde_json::from_str(invalid_json);
            let json_error = result.unwrap_err();
            let action_error: CliActionError = json_error.into();

            match action_error {
                CliActionError::JsonError(_) => (),
                _ => panic!("Expected JsonError variant"),
            }
        }

        #[test]
        fn test_csv_error() {
            let mut writer = csv::Writer::from_writer(vec![]);
            let result = writer.write_record(["test"]);
            assert!(result.is_ok());

            let inner_result = writer.into_inner();
            assert!(inner_result.is_ok());
        }

        #[test]
        fn test_unsupported_output_format() {
            let error = CliActionError::UnsupportedOutputFormat("invalid".to_string());
            let error_str = error.to_string();
            assert!(error_str.contains("ERROR: Unsupported output format"));
            assert!(error_str.contains("invalid"));
        }

        #[test]
        fn test_missing_required_argument() {
            let error = CliActionError::MissingRequiredArgument("--path".to_string());
            let error_str = error.to_string();
            assert!(error_str.contains("--path"));
        }

        #[test]
        fn test_tenant_not_found() {
            let error = CliActionError::TenantNotFound {
                identifier: "test".to_string(),
            };
            let error_str = error.to_string();
            assert!(error_str.contains("test"));
        }

        #[test]
        fn test_io_error_conversion() {
            let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
            let action_error: CliActionError = io_error.into();

            match action_error {
                CliActionError::IoError(_) => (),
                _ => panic!("Expected IoError variant"),
            }
        }

        #[test]
        fn test_business_logic_error() {
            let error = CliActionError::BusinessLogicError("custom error".to_string());
            let error_str = error.to_string();
            assert!(error_str.contains("custom error"));
        }
    }

    mod error_utils_tests {
        use pcli2::error_utils;

        #[test]
        fn test_create_user_friendly_error_keeps_status_messages_verbatim() {
            // A status code in the text is not a reason to rewrite the message: a
            // folder named "401-series" is not an authentication problem.
            for msg in [
                "HTTP Error: 401 Unauthorized",
                "403 Forbidden",
                "409 Conflict",
                "Connection error",
                "Folder '/Projects/401-series' not found",
            ] {
                assert_eq!(error_utils::create_user_friendly_error(msg), msg);
            }
        }

        #[test]
        fn test_create_user_friendly_error_not_found() {
            let error_msg = "Resource not found";
            let friendly_msg = error_utils::create_user_friendly_error(error_msg);
            assert!(friendly_msg.contains("Resource not found"));
        }

        #[test]
        fn test_create_user_friendly_error_timeout() {
            let error_msg = "Request timeout";
            let friendly_msg = error_utils::create_user_friendly_error(error_msg);
            assert!(friendly_msg.contains("timeout"));
        }

        #[test]
        fn test_is_retryable_error_timeout() {
            assert!(error_utils::is_retryable_error("Connection timeout error"));
        }

        #[test]
        fn test_is_retryable_error_service_unavailable() {
            assert!(error_utils::is_retryable_error("503 Service Unavailable"));
        }

        #[test]
        fn test_is_retryable_error_gateway() {
            assert!(error_utils::is_retryable_error("502 Bad Gateway"));
        }

        #[test]
        fn test_is_not_retryable_error() {
            assert!(!error_utils::is_retryable_error("Invalid argument"));
            assert!(!error_utils::is_retryable_error("Authentication failed"));
        }

        #[test]
        fn test_create_user_friendly_error_invalid_client() {
            let error_msg = "invalid_client";
            let friendly_msg = error_utils::create_user_friendly_error(error_msg);
            // The original text stays, the explanation is appended.
            assert!(friendly_msg.starts_with("invalid_client"));
            assert!(friendly_msg.contains("client ID or secret"));
        }

        #[test]
        fn test_create_user_friendly_error_invalid_grant() {
            let error_msg = "invalid_grant";
            let friendly_msg = error_utils::create_user_friendly_error(error_msg);
            assert!(friendly_msg.starts_with("invalid_grant"));
            assert!(friendly_msg.contains("Log in again"));
        }

        #[test]
        fn test_generic_error_passthrough() {
            let error_msg = "Some unknown error occurred";
            let friendly_msg = error_utils::create_user_friendly_error(error_msg);
            // For unknown errors, should return the original message
            assert_eq!(friendly_msg, error_msg);
        }
    }

    mod configuration_error {
        use super::*;

        #[test]
        fn test_failed_to_find_configuration_directory() {
            let error = ConfigurationError::FailedToFindConfigurationDirectory;
            let error_str = error.to_string();
            assert!(error_str.contains("configuration directory"));
        }

        #[test]
        fn test_failed_to_load_data() {
            let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
            let error = ConfigurationError::FailedToLoadData {
                cause: Box::new(io_error),
            };
            let error_str = error.to_string();
            assert!(error_str.contains("failed to load configuration"));
        }

        #[test]
        fn test_failed_to_write_data() {
            let io_error =
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied");
            let error = ConfigurationError::FailedToWriteData {
                cause: Box::new(io_error),
            };
            let error_str = error.to_string();
            assert!(error_str.contains("failed to write"));
        }

        #[test]
        fn test_missing_required_property_value() {
            let error = ConfigurationError::MissingRequiredPropertyValue {
                name: "api_base_url".to_string(),
            };
            let error_str = error.to_string();
            assert!(error_str.contains("api_base_url"));
        }

        #[test]
        fn test_formatting_error_conversion() {
            let format_error = FormattingError::UnsupportedOutputFormat("test".to_string());
            let error = ConfigurationError::FormattingError {
                cause: Box::new(format_error),
            };
            let error_str = error.to_string();
            assert!(error_str.contains("test"));
        }
    }

    mod formatting_error {
        use super::*;

        #[test]
        fn test_unsupported_output_format() {
            let error = FormattingError::UnsupportedOutputFormat("xml".to_string());
            let error_str = error.to_string();
            assert!(error_str.contains("xml"));
        }

        #[test]
        fn test_format_failure() {
            let io_error = std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid data");
            let error = FormattingError::FormatFailure {
                cause: Box::new(io_error),
            };
            let error_str = error.to_string();
            assert!(error_str.contains("failed to format"));
        }

        #[test]
        fn test_csv_error_conversion() {
            let csv_error = csv::Error::from(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "csv error",
            ));
            let format_error: FormattingError = csv_error.into();

            match format_error {
                FormattingError::CsvError(_) => (),
                _ => panic!("Expected CsvError variant"),
            }
        }

        #[test]
        fn test_utf8_error_conversion() {
            let invalid_utf8 = vec![0, 159, 146, 150];
            let result = String::from_utf8(invalid_utf8);
            assert!(result.is_err());

            let utf8_error = result.unwrap_err();
            let format_error: FormattingError = utf8_error.into();

            match format_error {
                FormattingError::Utf8Error(_) => (),
                _ => panic!("Expected Utf8Error variant"),
            }
        }

        #[test]
        fn test_json_serialization_error() {
            // Test that JSON serialization errors convert properly to FormattingError
            // Create an IO error wrapped in serde_json::Error
            let io_error = std::io::Error::new(std::io::ErrorKind::InvalidData, "test io error");
            let json_error = serde_json::Error::io(io_error);
            let format_error: FormattingError = json_error.into();

            match format_error {
                FormattingError::JsonSerializationError(_) => (),
                _ => panic!("Expected JsonSerializationError variant"),
            }
        }
    }
}
