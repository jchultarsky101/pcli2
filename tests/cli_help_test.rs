#[cfg(test)]
mod cli_help_tests {
    use assert_cmd::prelude::*;
    use std::process::Command;

    #[test]
    fn test_cli_help_output() {
        // Test that the CLI help command executes successfully and produces expected output
        let mut cmd = Command::cargo_bin("pcli2").unwrap();

        // Run the CLI with --help flag to get the help output
        let assert_result = cmd.arg("--help").assert().success();
        let output = assert_result.get_output();
        let help_output = String::from_utf8_lossy(&output.stdout);

        // Print the help output for manual verification
        println!("CLI Help Output:\n{}", help_output);

        // Verify that the help output contains expected elements
        assert!(help_output.contains("Usage:"));
        assert!(help_output.contains("Options:"));
        assert!(help_output.contains("Commands:"));

        // Verify that major command groups are present
        assert!(help_output.contains("tenant")); // tenant commands
        assert!(help_output.contains("folder")); // folder commands
        assert!(help_output.contains("asset")); // asset commands
        assert!(help_output.contains("auth")); // auth commands
                                               // Context command has been moved to tenant command
        assert!(help_output.contains("config")); // config commands

        // Verify that help flags are present
        assert!(help_output.contains("-h, --help"));
        assert!(help_output.contains("-V, --version"));

        // Verify that the application name appears in the help
        assert!(help_output.contains("pcli2"));
    }

    #[test]
    fn test_cli_subcommand_help_outputs() {
        // Test help output for each major subcommand
        let subcommands = vec!["tenant", "folder", "asset", "auth", "config"];

        for subcommand in subcommands {
            let mut cmd = Command::cargo_bin("pcli2").unwrap();
            let assert_result = cmd.arg(subcommand).arg("--help").assert().success();
            let output = assert_result.get_output();
            let help_output = String::from_utf8_lossy(&output.stdout);

            // Print the help output for manual verification
            println!("Help Output for '{}':\n{}", subcommand, help_output);

            // Verify that each subcommand help contains expected elements
            assert!(help_output.contains("Usage:"));
            assert!(help_output.contains(subcommand)); // The subcommand name should appear in its help

            // Each subcommand should have its own specific subcommands
            if subcommand == "tenant" {
                assert!(help_output.contains("list"));
                assert!(help_output.contains("get"));
            } else if subcommand == "folder" {
                assert!(help_output.contains("list"));
                assert!(help_output.contains("get"));
                assert!(help_output.contains("create"));
                assert!(help_output.contains("delete"));
            } else if subcommand == "asset" {
                assert!(help_output.contains("get"));
                assert!(help_output.contains("list"));
                assert!(help_output.contains("create"));
                assert!(help_output.contains("create-batch"));
                assert!(help_output.contains("delete"));
                assert!(help_output.contains("download"));
                assert!(help_output.contains("geometric-match"));
                assert!(help_output.contains("part-match"));
                assert!(help_output.contains("visual-match"));
                assert!(help_output.contains("metadata"));
                assert!(help_output.contains("dependencies"));
            } else if subcommand == "auth" {
                assert!(help_output.contains("login"));
                assert!(help_output.contains("logout"));
                assert!(help_output.contains("get"));
            } else if subcommand == "config" {
                assert!(help_output.contains("get"));
                assert!(help_output.contains("export"));
                assert!(help_output.contains("import"));
                assert!(help_output.contains("environment"));
            }
        }
    }

    #[test]
    fn test_cli_version_output() {
        // Test that the CLI version command executes successfully
        let mut cmd = Command::cargo_bin("pcli2").unwrap();

        // Run the CLI with --version flag to get the version output
        let assert_result = cmd.arg("--version").assert().success();
        let output = assert_result.get_output();
        let version_output = String::from_utf8_lossy(&output.stdout);

        // Print the version output
        println!("CLI Version Output: {}", version_output);

        // Verify that the version output contains the application name and version
        assert!(version_output.contains("pcli2"));
        assert!(version_output.contains('.')); // Should contain version numbers with dots
    }

    #[test]
    fn test_nested_subcommand_help() {
        // Test help output for nested subcommands
        let nested_commands = vec![
            ("tenant", "list"),
            ("tenant", "get"),
            ("folder", "list"),
            ("folder", "get"),
            ("folder", "create"),
            ("folder", "delete"),
            ("asset", "list"),
            ("asset", "get"),
            ("asset", "create"),
            ("asset", "create-batch"),
            ("asset", "delete"),
            ("asset", "download"),
            ("asset", "geometric-match"),
            ("asset", "part-match"),
            ("asset", "visual-match"),
            ("folder", "geometric-match"),
            ("folder", "part-match"),
            ("folder", "visual-match"),
            ("asset", "dependencies"),
            ("auth", "login"),
            ("auth", "logout"),
            ("auth", "get"),
            ("config", "get"),
            ("config", "export"),
            ("config", "import"),
        ];

        for (parent_cmd, sub_cmd) in nested_commands {
            let mut cmd = Command::cargo_bin("pcli2").unwrap();
            let assert_result = cmd
                .arg(parent_cmd)
                .arg(sub_cmd)
                .arg("--help")
                .assert()
                .success();
            let output = assert_result.get_output();
            let help_output = String::from_utf8_lossy(&output.stdout);

            // Print the help output for manual verification
            println!(
                "Help Output for '{} {}':\n{}",
                parent_cmd, sub_cmd, help_output
            );

            // Verify that each nested subcommand help contains expected elements
            assert!(help_output.contains("Usage:"));
            assert!(help_output.contains(parent_cmd));
            assert!(help_output.contains(sub_cmd));
        }
    }

    #[test]
    fn test_deeply_nested_subcommand_help() {
        // Test help output for deeply nested subcommands like asset metadata
        let deeply_nested_commands = vec![
            ("asset", "metadata", "get"),
            ("asset", "metadata", "create"),
            ("asset", "metadata", "delete"),
            ("asset", "metadata", "inference"),
            ("asset", "metadata", "create-batch"),
            ("config", "environment", "add"),
            ("config", "environment", "use"),
            ("config", "environment", "list"),
            ("config", "environment", "get"),
            ("config", "environment", "remove"),
            ("config", "environment", "reset"),
        ];

        for (parent_cmd, sub_cmd, sub_sub_cmd) in deeply_nested_commands {
            let mut cmd = Command::cargo_bin("pcli2").unwrap();
            let assert_result = cmd
                .arg(parent_cmd)
                .arg(sub_cmd)
                .arg(sub_sub_cmd)
                .arg("--help")
                .assert()
                .success();
            let output = assert_result.get_output();
            let help_output = String::from_utf8_lossy(&output.stdout);

            // Print the help output for manual verification
            println!(
                "Help Output for '{} {} {}':\n{}",
                parent_cmd, sub_cmd, sub_sub_cmd, help_output
            );

            // Verify that each deeply nested subcommand help contains expected elements
            assert!(help_output.contains("Usage:"));
            assert!(help_output.contains(parent_cmd));
            assert!(help_output.contains(sub_cmd));
            assert!(help_output.contains(sub_sub_cmd));
        }
    }
}
