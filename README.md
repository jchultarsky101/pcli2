# PCLI2

The goal of this project is to create version 2 of the Physna Command Line Interface client (PCLI2).

Based on lessons learned, we will develop a new and more ergonomic interface. It will operate more like Git's
excellent CLI utilizing nested sub-commands, sensible defaults and configuration.

## Commands

The application uses a hierarchy of commands and sub-commands. Type the command first, followed by a sub-command (if any), followed by the arguments. See examples below.

### Command "help"

This command prints the usage help. For example:

````
pcli2 help
````

The output will be similer to this:

````
CLI client utility to the Physna public API

Usage: pcli2 <COMMAND>

Commands:
  config   working with configuration
  folders  lists all folders
  login    attempts to login for this tenant
  logoff   attempts to logoff for this tenant
  help     Print this message or the help of the given subcommand(s)
````

You can see more detailed information for each of the commands by providing the command name:

````
pcli2 help config
````

````
working with configuration

Usage: pcli2 config <COMMAND>

Commands:
  show    displays configuration
  export  exports the current configuration as a Yaml file
  set     sets configuration property
  delete  delete
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help
  -V, --version  Print version
````

You can go one level further by providing the name of the sub-command:

````
pcli2 help config show
````

````
displays configuration

Usage: pcli2 config show [OPTIONS] [COMMAND]

Commands:
  path    show the configuration path
  tenant  shows tenant configuration
  help    Print this message or the help of the given subcommand(s)

Options:
  -f, --format <format>  Output data format [default: json] [possible values: json, csv]
  -h, --help             Print help
  -V, --version          Print versio
````

### Command "config"

This command is used to manage the configuration.

#### Command "config show"

 

...


