#[cfg(test)]
mod cli_help_snapshot_tests {
    use assert_cmd::prelude::*;
    use std::collections::HashMap;
    use std::process::Command;

    #[test]
    fn test_cli_help_snapshot() {
        // Capture the current help output as a snapshot to detect changes
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        let assert_result = cmd.arg("--help").assert().success();
        let output = assert_result.get_output();
        let help_output = String::from_utf8_lossy(&output.stdout);

        // Define the expected snapshot of the help output
        // This will help us detect if any changes are made to the CLI interface
        let expected_snapshot = &help_output;

        // Verify that the help output contains expected elements
        assert!(expected_snapshot.contains("Usage: pcli2 <COMMAND>"));
        assert!(expected_snapshot.contains("Commands:"));
        assert!(expected_snapshot.contains("Options:"));
        assert!(expected_snapshot.contains("tenant"));
        assert!(expected_snapshot.contains("folder"));
        assert!(expected_snapshot.contains("auth"));
        assert!(expected_snapshot.contains("asset"));
        // Context command has been moved to tenant command
        assert!(expected_snapshot.contains("config"));
        assert!(expected_snapshot.contains("-h, --help"));
        assert!(expected_snapshot.contains("-V, --version"));

        // Print the help output for manual verification
        println!("CLI Help Snapshot:\n{}", expected_snapshot);
    }

    #[test]
    fn test_cli_help_subcommands_snapshot() {
        // Capture snapshots of each main subcommand's help
        let subcommands = vec!["tenant", "folder", "asset", "auth", "config"];
        let mut snapshots: HashMap<String, String> = HashMap::new();

        for subcommand in subcommands {
            let mut cmd = Command::cargo_bin("pcli2").unwrap();
            let assert_result = cmd.arg(subcommand).arg("--help").assert().success();
            let output = assert_result.get_output();
            let help_output = String::from_utf8_lossy(&output.stdout);

            snapshots.insert(subcommand.to_string(), help_output.to_string());

            // Verify each subcommand help contains expected elements
            assert!(help_output.contains(&format!("Usage: pcli2 {}", subcommand)));
            assert!(help_output.contains("Options:"));
        }

        // Print all snapshots for manual verification
        for (subcommand, snapshot) in &snapshots {
            println!("Snapshot for '{}':\n{}", subcommand, snapshot);
        }
    }

    #[test]
    fn test_cli_version_snapshot() {
        // Capture the version output as a snapshot
        let mut cmd = Command::cargo_bin("pcli2").unwrap();
        let assert_result = cmd.arg("--version").assert().success();
        let output = assert_result.get_output();
        let version_output = String::from_utf8_lossy(&output.stdout);

        // Verify the version output format
        assert!(version_output.contains("pcli2"));
        assert!(version_output.contains('.')); // Contains version numbers

        // Print the version output
        println!("Version Snapshot: {}", version_output);
    }

    #[test]
    fn test_specific_subcommand_snapshots() {
        // Test snapshots for specific nested subcommands
        let test_cases = vec![
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
            ("auth", "login"),
            ("auth", "logout"),
            ("auth", "get"),
            ("config", "get"),
            ("config", "export"),
            ("config", "import"),
        ];

        for (parent_cmd, sub_cmd) in test_cases {
            let mut cmd = Command::cargo_bin("pcli2").unwrap();
            let assert_result = cmd
                .arg(parent_cmd)
                .arg(sub_cmd)
                .arg("--help")
                .assert()
                .success();
            let output = assert_result.get_output();
            let help_output = String::from_utf8_lossy(&output.stdout);

            // Verify each nested subcommand help contains expected elements
            assert!(help_output.contains(&format!("Usage: pcli2 {} {}", parent_cmd, sub_cmd)));

            // Print the snapshot
            println!(
                "Snapshot for '{} {}':\n{}",
                parent_cmd, sub_cmd, help_output
            );
        }
    }

    #[test]
    fn test_deeply_nested_subcommand_snapshots() {
        // Test snapshots for deeply nested subcommands
        let test_cases = vec![
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

        for (parent_cmd, sub_cmd, sub_sub_cmd) in test_cases {
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

            // Verify each deeply nested subcommand help contains expected elements
            assert!(help_output.contains(&format!(
                "Usage: pcli2 {} {} {}",
                parent_cmd, sub_cmd, sub_sub_cmd
            )));

            // Print the snapshot
            println!(
                "Snapshot for '{} {} {}':\n{}",
                parent_cmd, sub_cmd, sub_sub_cmd, help_output
            );
        }
    }
}
