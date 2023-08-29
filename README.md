# SIMBIoTA

**Similarity-Based IoT Antivirus**

[![GitHub Release][release-shield]](https://github.com/simbiota/simbiota/releases)
[![GitHub Stars][stars-shield]](https://github.com/simbiota/simbiota/stargazers)
[![GitHub License][license-shield]](https://github.com/simbiota/simbiota/blob/master/LICENSE)
[![Downloads][download-shield]](https://github.com/simbiota/simbiota/releases)
[![Build Status][build-status-shield]](https://github.com/simbiota/simbiota/actions)
[![Website][website-shield]](https://simbiota.io)

The main goal of the project is to provide a lightweight (both memory and CPU usage) antivirus mainly for resource-constrained IoT devices.

**The project is currently in ALPHA stage.**

**It may detect some benign files and if quarantine is enabled, move them from their original location.
This may render your device inoperable and you need to manually move the file back from the quarantine to its original location.
Please create and issue if you find a false positive detection.**

## Installation

Currently we provide packages for Raspberry Pi devices, but we will soon create packages for other linux distributions as well.
In order for Simbiota to be able to block access to malicious files `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` kernel config needs to be set
in your running kernel. If the config is disabled, Simbiota still works but it will only alert on detection, it won't block accessing
the malicious file.
On stock Raspbian systems, this config is disabled.

You can check that your running kernel has `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` enabled with the following script:

```bash
modprobe configs
if zcat /proc/config.gz | grep -q CONFIG_FANOTIFY_ACCESS_PERMISSIONS=y;
then
    echo "FANOTIFY_ACCESS_PERMISSIONS are enabled, you can run Simbiota";
else
    echo "Kernel needs to be recompiled to support Simbiota";
fi
```

If you want to recompile the kernel, you can follow the instructions in [installation.md](installation.md) section.

If it is, than all you need to do is follow our [compilation guide](#build-yourself).

### Install the released package (Raspberry Pi (arm64/armv7))

Figure out whether you need the `arm64` version or the `armv7l` version of the package with the following commands:

```bash
arch=$(uname -m)
if [[ ${arch} == "aarch64" ]];
then
    echo "You are running an arm64 kernel, please use simbiota_0.0.1_arm64.deb.";
elif [[ ${arch} == "armv7l" ]];
then
    echo "You are running an armv7l kernel, please use simbiota_0.0.1_armv7.deb.";
else
    echo "You are running a ${arch} kernel, we currently don't provide releases for this architecture. Please recompile Simbiota to use it on your device.";
fi
```

If your device is ready, download the latest release of Simbiota from the [releases page](https://github.com/simbiota/simbiota/releases),
then proceed to installing Simbiota:

```bash
apt install simbiota_0.0.1_arm64.deb
```

### Build yourself

You can probably try Simbiota on any linux system, you just need to compile it.
Simbiota is written in Rust, so you need to setup a working rust environment with either `rust` or `rustup`
follow your distribution's documentation. When it's ready, you can build Simbiota with the following commands:

```bash
git clone https://github.com/simbiota/simbiota.git
cd simbiota
cargo build --release --target=<your-rust-target>
# you can list all rust targets with `rustup target list`
```

## Usage

### Database

**In order to use Simbiota, you need a detection database.**
You may either use `simbiota-update.timer` systemd timer to download the latest database release every day.

Or update the database with our provided script `/usr/sbin/simbiota-update.sh` (first read it, then run it as root).

Or download one from our [`database-releases`](https://github.com/simbiota/database-releases/releases) page.
Replace `<arch>` and `<version>` with the appropriate values found on the releases page.
```bash
curl -L https://github.com/simbiota/database-releases/releases/download/<version>/simbiota-<arch>-<version>.sdb -o /var/lib/simbiota/database.sdb
```
Configure Simbiota to use this database by setting the `database.database_file` key in your config.
```
# /etc/simbiota/client.yaml
---
...
database:
  database_file: /var/lib/simbiota/database.sdb
...
```

### Service

The recommended way of running Simbiota is through the `simbiota.service` systemd service.
Enable and start it with the following command:

```bash
systemctl enable simbiota.service
systemctl start simbiota.service
```

### Arguments

Check out `man simbiota` and `man simbiota_config` man pages for details.

```
Usage: simbiota [OPTIONS]

Options:
  -c, --config <FILE>  Specify a custom config file
      --bg             Run in daemon mode
  -v, --verbose        Verbose output
  -h, --help           Print help
```

You can manipulate files in the quarantine with `simbiotactl`.

```
Usage: simbiotactl <COMMAND>

Commands:
  quarantine  Manual scan operations Quarantine operations
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

### Installed from .deb

If you installed Simbiota from our released `.deb` package you should have the following files created:
```
/etc/simbiota/client.yaml
/usr/lib/systemd/system/simbiota.service
/usr/lib/systemd/system/simbiota-update.service
/usr/lib/systemd/system/simbiota-update.timer
/usr/sbin/simbiota
/usr/sbin/simbiotactl
/usr/sbin/simbiota-update.sh
/usr/share/bash-completion/completions/simbiotactl
/usr/share/fish/completions/simbiotactl.fish
/usr/share/man/man5/simbiota_config.5.gz
/usr/share/man/man8/simbiota.8.gz
/usr/share/zsh/vendor-completions/_simbiotactl
```

You should edit the config file at `/etc/simbiota/client.yaml` to your liking.
Then you can start Simbiota by either a service:
```bash
systemctl start simbiota.service
```
or as a standalone program as well
```bash
/usr/sbin/simbiota
```

### Manual build

After you successfully built Simbiota, you can find the programs at `./target/*/release/{simbiota,simbiotactl}`.

Use the config file located at `./package/common/etc/simbiota/client.yaml`.

## How it works?

### fanotify

Simbiota uses [fanotify](https://man7.org/linux/man-pages/man7/fanotify.7.html) to trigger detection.
Fanotify provides a user process ability to insert marks on filesystem objects in order to get notifications when the
event determined by the specific mark is triggered. When `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` is enabled in the
current kernel configuration, the API also provides the ability for the process to determine whether the specific
filesystem access event should be allowed to go through or should be blocked.
We use this API to place the selected marks (specified in the configuration file) on filesystem objects (optionally the whole `/` filesystem)
to get notifications when a file is accessed, opened, opened for execution etc.
When a notification arrives, we ask the configured detection engines to determine if the target file is a malware or not.
If it is a malware, the operation is blocked and (if configured), the file is moved to quarantine.

### Detection engine

This client detects malware samples based on the currently used [database](https://github.com/simbiota/database-releases).
The current implementation uses a single detector, the [TLSH](https://github.com/trendmicro/tlsh) detector that
1. calculates the TLSH hash of the scanned sample
2. compares it to every sample in its database
3. returns positive detection if the TLSH difference is bellow a threshold (default: 40)

We based our first detector on TLSH, because through extensive testing we found that
- it detects binary similarity particularly well
- hash calculation and comparison is very fast

This detector uses the TLSH object in the database.
This object (as described on the [database page](https://github.com/simbiota/database)) stores TLSH hashes associated with malware samples.
The samples are selected in such a way, that every malware sample in the backend is similar to at least one selected sample.
In other words, the samples in the database form a dominating set in the graph, where nodes are samples and two of them are connected if
their TLSH difference score is bellow the threshold.

### Cache

When a file is first scanned, the TLSH detector calculates its TLSH hash.
This requires reading the entire file and so the blocking operation can be slow (~10-100ms).
The actual delay depends on the storage media.

In order to minimize the delay when a file is scanned again, we use a caching mechanism.
We cache the detection results for every file scanned.
Further detection results will come from the cache only if the following stat data of the file remains the same:
`size`, `uid`, `gid`, `mtime`, `ctime`, `mode`.
Fow each file, only 48 bytes are stored in the cache, thus it remains quite small.
A cache result is delivered in the ~10-100us range, much faster than without caching.

### Performance

The evaluation bellow was performed on a `Raspberry Pi 4 Model B Rev 1.2`.

### Scanning delay

The delay from starting to scan a file till the detection result's arrival consists of the following parts:
1 calculating the TLSH hash of the file
  - our [tlsh-rust](https://github.com/simbiota/tlsh-rust) implementation currently calculates TLSH hashes at 20MBps
2. comparing the hash to every sample in the database
  - takes `~4µs` per comparison

So for example the average delay for scanning a `1455120` byte long `libc` on our device for the first time with `60000` samples in the database takes `0.069 + 0.24 = 0.309ms`.
Later scanning delays would take around `100-200µs` from cache.

### Memory usage

Memory usage of Simbiota currently adds up from 3 parts:
- `simbiota` binary itself: `~2.6MB`
- used libraries (`~1.1MB`):
  - `libc-2.31.so`: 1455120
  - `libdl-2.31.so`: 14560
  - `libm-2.31.so`: 633000
  - `libpthread-2.31.so`: 160200
  - `ld-2.31.so`: 145352
- database:
  - our ARM database is corrently `4.5MB`
  - we will reduce this with advanced filtering
This sums up to `~8-10MB` and only increases with the cache, that stores 48 bytes for each cached sample.
Our test Raspberry Pi currently has `156713` files on it, if all of them are in the cache, they occupy `~7.5MB`.
```console
$ sudo find / -type f -not -path "/dev*" -not -path "/proc*" -not -path "/sys*" -not -path "/run*" | wc -l
156713
```


[release-shield]: https://img.shields.io/github/release/simbiota/simbiota
[stars-shield]: https://img.shields.io/github/stars/simbiota/simbiota
[license-shield]: https://img.shields.io/github/license/simbiota/simbiota
[download-shield]: https://img.shields.io/github/downloads/simbiota/simbiota/total
[build-status-shield]: https://img.shields.io/github/actions/workflow/status/simbiota/simbiota/package.yml?branch=master

[website-shield]: https://img.shields.io/badge/-Website-informational.svg?color=000000&logo=data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAADEAAAARCAMAAABD7rQYAAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAHOUExURQAAADOVjzSWjzKUjjeXkzOUjjeZkzOWkDSVjzKUjTOWkTSbkzOVjTOVkDOYkzOVjjKWjzKUjzaUkjKWkDKVjzaVkjSWkDKVjjOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVjzOVj////0UuoloAAACZdFJOUwAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAACJdcWpkaWIqBUkuCwkoXlJXSwJAYApIPlUBIGMEExYVB1w3FyUUD0MSOgMjTysNWzA/mYCBk0KVJ3s9gokxpXp0lmaHoDRWDFE4CH48qZp9w6J4fxibbFAzbyEfi4OhhJEcVGVnp69odYptpFhwShAsNk0kRS87nw6Sa18RHhkdKXLX9dgAAAABYktHRJkB1jaoAAAACXBIWXMAAAsTAAALEwEAmpwYAAAAB3RJTUUH5wYdDgsZZ6eneQAAAepJREFUKM+Nk/tX0mAYx/fN7je7h6PGCBBoXHTgBiITEDeM2czCCyYoWSvFmLfupZaKldm69+c2PHaYp070/PKe5z3v5zyX7/clCACWFtJ66TJJ2fYBTUSjAGg7dcUB7He6Wt1OoDFBe7w0cAAHgauMz98YQcCLegTb2k3Zod3zsOkOBBsyKiDcwfE8x0eORG1G0umNdcW5o0Bc6A7wiS5Y3MlUTzrdC04EIWUMDH3XsrIs9/tgkY7hutIycGPwpguxW7mh4SwzkhnNj+V7R2+PQy6AIIs4TmAimopEwkkSdEjApGIv3Zm6e09Q7+cekCozPcOg2F+eoMDOjvmJh+3GqKgUai1qc4BnHlgYtqrBxaVHS7PaUP6xd1pOovTk6bPnePHyVTexvDOOQRjcuEFUBtAZza6owVXba5eivaGUysiksqauoy0krmY2Fms1SlPltLkGG3VXN7mcE+G31XeF97wNm61bJSz0CB9AbxOkiI2PMwWpPscJXowViw5R0PWU7vDrepzbs11jV5lPPqa+q5PNduqzFpDINZ+kzVnXVzxfAqfMstb0EFO/9Wiq6fH1W1nQv88nYj+q7GAfm0h07DHCvzQ392Iiwju+Oo0zf/HVWaD5D5/tevcczv+vd83/4+cF4GKj978AriFv2eu+51MAAAAldEVYdGRhdGU6Y3JlYXRlADIwMjMtMDYtMjlUMTM6NTE6MzIrMDA6MDAGPwK2AAAAJXRFWHRkYXRlOm1vZGlmeQAyMDIzLTA2LTI5VDEzOjUxOjMyKzAwOjAwd2K6CgAAACh0RVh0ZGF0ZTp0aW1lc3RhbXAAMjAyMy0wNi0yOVQxNDoxMToyNSswMDowMOTQ0B8AAAAASUVORK5CYII=