use color_print::cprintln;

/// Display the ASCII art banner for PCLI2
pub fn print_banner() {
    cprintln!("<r>PCLI2</r> - Physna Command Line Interface v2\n");
    
    // Print ASCII art spelling out "PCLI2" with gradient orange colors
    cprintln!("<y> ███████████    █████████  █████       █████  ████████ </y>");
    cprintln!("<#FFA500>░░███░░░░░███  ███░░░░░███░░███       ░░███  ███░░░░███</#FFA500>");
    cprintln!("<#FFA500> ░███    ░███ ███     ░░░  ░███        ░███ ░░░    ░███</#FFA500>");
    cprintln!("<y> ░██████████ ░███          ░███        ░███    ███████ </y>");
    cprintln!("<y> ░███░░░░░░  ░███          ░███        ░███   ███░░░░  </y>");
    cprintln!("<#FF8C00> ░███        ░░███     ███ ░███      █ ░███  ███      █</#FF8C00>");
    cprintln!("<#FF8C00> █████        ░░█████████  ███████████ █████░██████████</#FF8C00>");
    cprintln!("<#FF8C00>░░░░░          ░░░░░░░░░  ░░░░░░░░░░░ ░░░░░ ░░░░░░░░░░ </#FF8C00>");
    println!();
}

/// Check if the command line arguments include help request
pub fn has_help_flag(args: &[String]) -> bool {
    for arg in args.iter() {
        if arg == "--help" || arg == "-h" {
            return true;
        }
    }
    false
}