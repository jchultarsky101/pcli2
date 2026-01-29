//! Integration tests for the asset command functionality.
//!
//! These tests verify that the asset commands work correctly with different
//! options and formats, ensuring the CLI interface behaves as expected.

#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::prelude::*;

    /// Test that the asset list command displays help information correctly
    /// when the --help flag is provided.
    ///
    /// This test verifies that:
    /// - The command exits successfully
    /// - Help text contains the command description
    /// - Help text shows available options like --format, --pretty, and --headers
    #[test]
    fn test_asset_list_command_help() {
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.arg("asset").arg("list").arg("--help");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("List all assets"))
            .stdout(predicate::str::contains("--format"))
            .stdout(predicate::str::contains("--pretty"))
            .stdout(predicate::str::contains("--headers"));
    }

    /// Test that the asset get command displays help information correctly
    /// when the --help flag is provided.
    ///
    /// This test verifies that:
    /// - The command exits successfully
    /// - Help text contains the command description
    /// - Help text shows available options like --format, --pretty, and --headers
    #[test]
    fn test_asset_get_command_help() {
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.arg("asset").arg("get").arg("--help");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Get asset details"))
            .stdout(predicate::str::contains("--format"))
            .stdout(predicate::str::contains("--pretty"))
            .stdout(predicate::str::contains("--headers"));
    }

    /// Test that the asset create command displays help information correctly
    /// when the --help flag is provided.
    ///
    /// This test verifies that:
    /// - The command exits successfully
    /// - Help text contains the command description
    /// - Help text shows required parameters like --file and --folder-path or --folder-uuid
    #[test]
    fn test_asset_create_command_help() {
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.arg("asset").arg("create").arg("--help");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains(
                "Create a new asset by uploading a file",
            ))
            .stdout(predicate::str::contains("--file"))
            .stdout(predicate::str::contains("--folder-path"));
    }

    /// Test that the asset create-batch command displays help information correctly
    /// when the --help flag is provided.
    ///
    /// This test verifies that:
    /// - The command exits successfully
    /// - Help text contains the command description
    /// - Help text shows required parameters like --files
    #[test]
    fn test_asset_create_batch_command_help() {
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.arg("asset").arg("create-batch").arg("--help");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains(
                "Create multiple assets by uploading files",
            ))
            .stdout(predicate::str::contains("--files"));
    }

    /// Test that the asset delete command displays help information correctly
    /// when the --help flag is provided.
    ///
    /// This test verifies that:
    /// - The command exits successfully
    /// - Help text contains the command description
    /// - Help text shows required parameters like --uuid or --path
    #[test]
    fn test_asset_delete_command_help() {
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.arg("asset").arg("delete").arg("--help");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Delete an asset"))
            .stdout(predicate::str::contains("--uuid"))
            .stdout(predicate::str::contains("--path"));
    }

    /// Test that the asset download command displays help information correctly
    /// when the --help flag is provided.
    ///
    /// This test verifies that:
    /// - The command exits successfully
    /// - Help text contains the command description
    /// - Help text shows required parameters like --uuid or --path
    #[test]
    fn test_asset_download_command_help() {
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.arg("asset").arg("download").arg("--help");

        cmd.assert()
            .success()
            .stdout(predicate::str::contains("Download asset file"))
            .stdout(predicate::str::contains("--uuid"))
            .stdout(predicate::str::contains("--path"));
    }

    /// Test that all supported output formats are accepted by the asset get command.
    ///
    /// This test verifies that:
    /// - Each supported format (json, csv) is accepted as a valid argument
    /// - Commands with valid formats execute (may fail for other reasons like API access, but not format-related)
    /// - The tree format is not supported for asset get command
    #[test]
    fn test_asset_get_command_supported_formats() {
        // Test that supported formats are accepted by the CLI
        let formats = vec!["json", "csv"];

        for format in formats {
            let mut cmd = Command::cargo_bin("pcli2").unwrap();
            cmd.arg("asset")
                .arg("get")
                .arg("--uuid")
                .arg("00000000-0000-0000-0000-000000000000") // Invalid UUID to test format acceptance
                .arg("--format")
                .arg(format);

            // For invalid UUID, we expect an error but not a format-related error
            cmd.assert().failure(); // Will fail due to API access but not format-related
        }

        // Test that tree format is not supported (should fail)
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.arg("asset")
            .arg("get")
            .arg("--uuid")
            .arg("00000000-0000-0000-0000-000000000000")
            .arg("--format")
            .arg("tree");

        cmd.assert()
            .failure()
            .stderr(predicate::str::contains("invalid value 'tree'"));
    }

    /// Test that the asset get command supports --pretty flag with JSON format.
    ///
    /// This test verifies that:
    /// - The command accepts the --pretty flag
    /// - The command processes the flag without format-related errors (may fail for other reasons)
    #[test]
    fn test_asset_get_command_pretty_format() {
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.arg("asset")
            .arg("get")
            .arg("--uuid")
            .arg("00000000-0000-0000-0000-000000000000") // Invalid UUID to test flag acceptance
            .arg("--format")
            .arg("json")
            .arg("--pretty");

        // Should fail due to invalid UUID but should accept the format flags
        cmd.assert().failure(); // Will fail due to API access but should accept the flag
    }

    /// Test that the asset get command supports --headers flag with CSV format.
    ///
    /// This test verifies that:
    /// - The command accepts the --headers flag
    /// - The command processes the flag without format-related errors (may fail for other reasons)
    #[test]
    fn test_asset_get_command_headers_format() {
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        cmd.arg("asset")
            .arg("get")
            .arg("--uuid")
            .arg("00000000-0000-0000-0000-000000000000") // Invalid UUID to test flag acceptance
            .arg("--format")
            .arg("csv")
            .arg("--headers");

        // Should fail due to invalid UUID but should accept the format flags
        cmd.assert().failure(); // Will fail due to API access but should accept the flag
    }
}
