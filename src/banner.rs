use color_print::cprintln;

/// Display the ASCII art banner for PCLI2
pub fn print_banner() {
    println!();
    cprintln!("<r>PCLI2</r> - Physna Command Line Interface v2");
    println!();
    println!();
    
    // Print ASCII art spelling out "PCLI2" with smooth gradient from light to dark orange (top to bottom)
    cprintln!("<#FFA500> ███████████    █████████  █████       █████  ████████ </#FFA500>");
    cprintln!("<#FF9500>░░███░░░░░███  ███░░░░░███░░███       ░░███  ███░░░░███</#FF9500>");
    cprintln!("<#FF8C00> ░███    ░███ ███     ░░░  ░███        ░███ ░░░    ░███</#FF8C00>");
    cprintln!("<#FF8000> ░██████████ ░███          ░███        ░███    ███████ </#FF8000>");
    cprintln!("<#FF7200> ░███░░░░░░  ░███          ░███        ░███   ███░░░░  </#FF7200>");
    cprintln!("<#FF6300> ░███        ░░███     ███ ░███      █ ░███  ███      █</#FF6300>");
    cprintln!("<#FF5500> █████        ░░█████████  ███████████ █████░██████████</#FF5500>");
    cprintln!("<#FF4500>░░░░░          ░░░░░░░░░  ░░░░░░░░░░░ ░░░░░ ░░░░░░░░░░ </#FF4500>");
    println!();
    println!();
}

/// Check if the command line arguments include help request
pub fn has_help_flag(args: &[String]) -> bool {
    for arg in args.iter() {
        if arg == "--help" || arg == "-h" || arg == "help" {
            return true;
        }
    }
    false
}