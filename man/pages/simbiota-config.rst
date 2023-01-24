SIMBIOTA_CONFIG(5)
==================

DESCRIPTION:
------------

The configuration for SIMBIoTA Client is stored in a file (default location: ``/etc/simbiota/client.yaml``), in YAML format.
The file contains the configuration of :manpage:`simbiota(8)`.

YAML is a hierarchical format containing key-value pairs and it is easy to read and write for humans. The default configuration
contains sensible defaults for the program, but requires the user to provide email and update server information.

The following sections are used to configure the program:

``detector``
    Configuration of the detection engine used by `simbiota(8)`.

    The detector class specifies which detector is used. Additional options for the detector can be configured, ``simple_tlsh`` detector requires the ``threshold`` option with the specified TLSH
    distance to use (default: 40).

    The following options are awailable for the detector config:

        - ``class``: Detector to use. Currently only ``simple_tlsh`` is available.
        - ``config``: Additional options for the detector.

    Example detector configuration::

        detector:
            class: simple_tlsh
            config:
                threshold: 40

``monitor``
    Configuration for the filesystem monitoring.

    The client monitors the specified paths using ``fanotify(7)``. The paths are listed as ``path`` objects inside the ``paths`` key. Each ``path`` object can contain additional information about each monitored path, such as ``fanotify(7)`` flags.
    The following options can be used for path objects:

        - ``dir``: Mark path as directory, ``fanotify(7)`` will watch for events on children. (default: false)
        - ``event_on_children``: Events for the immediate children of marked directories shall be created. note that this is not recursive.
          If you mark ``/usr`` with ``dir: true`` and ``event_on_children: true`` executing ``/usr/bin/ls`` will not trigger an event. (default: false)
        - ``mount``: All directories, subdirectories, and the contained files of the mount will be monitored. (default: false)
        - ``filesystem``: Mark the entire filesystem on which the current path resides, ``fanotify(7)`` will watch for events on the entire filesystem. (default: false)
        - ``mask``: Specify the ``fanotify(7)`` masks used for see valid values in :manpage:`fanotify\_mark(2)` ``flags`` value.

    Example monitor config::

        monitor:
            paths:
                - path: "/usr/bin"
                dir: true
                event_on_children: true
                mask:
                    - OPEN_EXEC_PERM
                - path: "/usr/sbin"
                dir: true
                event_on_children: true
                mask:
                    - OPEN_EXEC_PERM

``email``
    Email alert configuration.

        - ``enabled``: Enable or disable email alerts

    The following values must be provided if you enable email alerts:

        - ``recipients``: List of email addresses to send alerts to.
        - ``smtp``: SMTP server configuration:
            
            - ``server``: SMTP server address.
            - ``port``: SMTP server port.
            - ``username``: SMTP login username.
            - ``password``: SMTP login password.
            - ``security``: Can be None, SSL, STARTTLS.
    
    Example email configuration::

        email:
            enabled: true
            recipients:
                - test1@domain.com
                - test2@domain2.com

            smtp:
                server: mail.example.com
                port: 587
                username: noreply@example.com
                password: SuperS3cret
                security: STARTTLS

``logger``
    List of logger outputs, can be empty if no logging is required. Each logger config is a logger object inside the list.

    The following options are required for logger objects:

        - ``output``: Logger output device. The following values can be used:
        
            - ``console``: Output log messages to the console
            - ``file``: Output log messages to a file
            - ``syslog``: Output log messages to a syslog server
        
        - ``level``: Log level for this output


``cache``
    Result caching options.

    The detector caches the detection result for faster detection times. It stores the file modification metadata with the result and check whether the file was modified since the last scan.

        - ``disable``: Disable detection result caching.


``database``
    Detection database options.

    - ``database_file``: Location of the database file.


``quarantine``
    Threat quarantine options:

    - ``enabled``: Enable or disable the quarantine functionality.
    - ``path``: Path of the quarantine directory.

    
SEE ALSO:
---------

:manpage:`simbiota(8)`