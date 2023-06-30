SIMBIOTA(8)
===========

SYNOPSIS:
---------
**simbiota-clientd** [-h] [-c config_file] [--bg] [--config config_file] [--verbose]

Description:
------------
:program:`simbiota` (SIMBIoTA Client Daemon) is the daemon program for SIMBIoTA. The
daemon listens for filesystem events using Linux's :manpage:`fanotify(7)` API and scans files on demand.

On startup, the daemon loads the configuration from a configuration file, which defaults to
``/etc/simbiota/client.yaml`` and can be overridden with the ``-c`` or ``--config`` command line option. The program
then listens to file actions on the directories specified in the configuration file (see :manpage:`simbiota_config(8)`.

When a file event occurs, the daemon reads the file's contents and scans it using the configured detector. If the detection is
positive, the file access is blocked, and the file is moved to a special quarantine directory, preventing further executions.

The program supports sending email notifications to specified addresses when a threat is detected. This feature can be enabled
in the configuration file (see :manpage:`simbiota_config(8)`, and requires a working SMTP server.

The daemon is designed to use as little resources as possible. It is written in Rust to be as secure as possible, and blocks the file accesses only for
the shortest time possible to scan the file. To access :manpage:`fanotify(7)`, the program requires the ``CAP_SYS_ADMIN``, meaning
the daemon must be run as root.

Options:
--------

.. program:: simbiota

.. option:: -h, --help

    Show a help message and exit.

.. option:: -c config_file, --config config_file

    Specify the configuration file to use. The default is /etc/simbiota/client.yaml.

.. option:: --bg

    Start the daemon in the background.

.. option:: --verbose

    Print verbose messages to the console. Useful for debugging.

SEE ALSO:
---------

:manpage:`simbiota_config(5)`