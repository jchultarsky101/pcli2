use color_print::cprintln;
use pcli2::terminal::colors_enabled;

/// Display the ASCII art banner for PCLI2
pub fn print_banner() {
    if colors_enabled() {
        print_banner_colored();
    } else {
        print_banner_plain();
    }
}

/// Banner with a smooth gradient from light to dark orange (top to bottom)
fn print_banner_colored() {
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

/// Banner without ANSI escape codes for non-TTY output or when colors are disabled
fn print_banner_plain() {
    println!();
    println!("PCLI2 - Physna Command Line Interface v2");
    println!();
    println!();
    println!(" ███████████    █████████  █████       █████  ████████ ");
    println!("░░███░░░░░███  ███░░░░░███░░███       ░░███  ███░░░░███");
    println!(" ░███    ░███ ███     ░░░  ░███        ░███ ░░░    ░███");
    println!(" ░██████████ ░███          ░███        ░███    ███████ ");
    println!(" ░███░░░░░░  ░███          ░███        ░███   ███░░░░  ");
    println!(" ░███        ░░███     ███ ░███      █ ░███  ███      █");
    println!(" █████        ░░█████████  ███████████ █████░██████████");
    println!("░░░░░          ░░░░░░░░░  ░░░░░░░░░░░ ░░░░░ ░░░░░░░░░░ ");
    println!();
    println!();
}
