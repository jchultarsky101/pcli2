use color_print::cprintln;

/// Display the ASCII art banner for PCLI2
pub fn print_banner() {
    cprintln!("<r>PCLI2</r> - Physna Command Line Interface v2\n");
    
    // Print ASCII art with gradient orange colors
    cprintln!("<y>    ____  ______   _______  _______  _______  _______  _______  ______  </y>");
    cprintln!("<y>   / ___)(__  _ \\(  ____ \\(  ____ \\(  ____ \\(  ___  )(  ___  )(  __  \\ </y>");
    cprintln!("<#FFA500>   \\/___ \\  | ( \\/| (    \\/| (    \\/| (    \\/| (   ) || (   ) || (  \\  \\</#FFA500>");
    cprintln!("<#FFA500>       __) | | (__ | (__    | (_____ | (_____ | |   | || |   | || |   ) |</#FFA500>");
    cprintln!("<#FF8C00>      / __/  |  __)|  __)   (_____  )(_____  )| |   | || |   | || |   | |</#FF8C00>");
    cprintln!("<#FF8C00>     / / ___ | (   | (            ) |      ) || |   | || |   | || |   ) |</#FF8C00>");
    cprintln!("<#FF7F50>    / /(___)| (___ | (____/\\/\\____) |_____) || (___) || (___) || (__/  )</#FF7F50>");
    cprintln!("<#FF7F50>    \\_______(_____/\\_______\\(________(_______(_______(_______(______/ </#FF7F50>");
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