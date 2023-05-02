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

This command is used to manage the configuration. It can display details about the current configuration, but also allows you to modify it by adding new configuration elements and deleting existing ones.
The data is stored in two locations:

* System-specific user configuration directory
* System-specific secure keyring

The configuration directory location depends on your operating system. You can use the **path** sub-command to display the location It can display details about the current configuration, but also allows you to modify it by adding new configuration elements and deleting existing ones.

The data is stored in two locations:

* System-specific user configuration directory
* System-specific secure keyring

The configuration directory location depends on your operating system. You can use the **path** sub-command to display the location. You should not need to deal with the configuration file itself. The CLI provides commands to manipulate the data for you.

Confidential entries such as OpenID Connect client secrets or your access tokens are stored in your operating system keyring. This is a protected area and the data is encrypted. You will be prompted to enter your user password from time to time when using it.

#### Command "config show"

This command outputs different elements of your configuration depending on what sub-command is used.

If you use it without any sub-commands, it will print out the full list of all tenant configurations currently present:

````
pcli2 confg show
````

The output may look as shown below. Please, note that this is just an example. Your configuration would look different.

````
{
  "tenants": {
    "my-first-tenant": {
      "tenant_id": "my-first-tenant",
      "api_url": "https://api.physna.com/",
      "oidc_url": "https://physna.okta.com/oauth2/default/v1/token",
      "client_id": "1..."
    }
    "my-second-tenant": {
      "tenant_id": "my-second-tenant",
      "api_url": "https://api.physna.com/",
      "oidc_url": "https://physna.okta.com/oauth2/default/v1/token",
      "client_id": "2..."
    }
  }
}
````

The **show** command takes an argument **--format**, which may have one of the following values: **json** or **csv**. The default is **json**. If you would like to output your configuration in CSV format instead, you can use the following:

````
pcli2 config show --format=csv
````

The **--format** argument is inherited by all sub-commands of **show**. More on that when we discuss the sub-command **tenant**. 

##### Command "config show path"

````
pcli2 config show path
````

This command will output the configuration directory path on your system. For exampmyuser

where "myuser" would be your file

The configuration iteself is a text file in Yaml format. You can view it with any text editor. However, it is best to modify it via the CLI and not directly to avoid issue. If you do, please make a backup copy first.

 MacOS it may be similar to this:

````
````


````
pcli2 config show path
````

This command will output the configuration file path on your system. For example, on MacOS it may be similar to this:

````
/Users/myuser/Library/Application Support/pcli2/config.yml
````

where "myuser" would be your username.

The configuration iteself is a text file in Yaml format. You can view it with any text editor. However, it is best to modify it via the CLI and not directly to avoid issue. If you do, please make a backup copy first.






 

...


