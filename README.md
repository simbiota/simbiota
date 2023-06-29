# SIMBIoTA

**Similarity-Based IoT Antivirus**

[![GitHub Release][release-shield]](https://github.com/simbiota/simbiota/releases)
[![GitHub Stars][stars-shield]](https://github.com/simbiota/simbiota/stargazers)
[![GitHub License][license-shield]](https://github.com/simbiota/simbiota/blob/master/LICENSE)
[![Downloads][download-shield]](https://github.com/simbiota/simbiota/releases)
[![Build Status][build-status-shield]](https://github.com/simbiota/simbiota/actions)
[![Website][website-shield]](https://simbiota.io)

The main goal of the project is to provide a lightweight (both memory and CPU usage) antivirus mainly for resource-constrained IoT devices.

## Installation

Currently we provide packages for Raspberry Pi devices, but we will soon create packages for other linux distributions as well.
If you are on a different system, check that you run a kernel with `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` enabled.
If it is, than all you need to do is follow our [compilation guide](#build-yourself).

### Install the released package (Raspberry Pi (arm64/armv7))

1. Check that your running kernel has `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` enabled:

```bash
modprobe configs
if zcat /proc/config.gz | grep -q CONFIG_FANOTIFY_ACCESS_PERMISSIONS=y;
then
    echo "FANOTIFY_ACCESS_PERMISSIONS are enabled, you can run Simbiota";
else
    echo "Kernel needs to be recompiled to support Simbiota";
fi
```

If you need to recompile the kernel, you can follow the instructions in [installation.md](installation.md) section.
Otherwise, if your device is ready, proceed to installing Simbiota:

```bash
dpkg -i simbiota_0.0.1_arm64.deb
```

### Build yourself

You can probably try Simbiota on any linux system with `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` enabled, you just need to compile it.
Simbiota is written in Rust, so you need to setup a working rust environment with either `rust` or `rustup`.

```bash
git clone https://github.com/simbiota/simbiota.git
cd simbiota
cargo build --release --target=<your-rust-target>
# you can list all rust targets with `rustup target list`
```

## Usage

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
/usr/lib/systemd/system/simbiota.service
/usr/sbin/simbiota
/usr/sbin/simbiotactl
/usr/share/zsh/vendor-completions/_simbiotactl
/usr/share/fish/completions/simbiotactl.fish
/usr/share/bash-completion/completions/simbiotactl
/etc/simbiota/client.yaml
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
to get notifications when a file is accessed, opened, opened for execution.
When a notification arrives, we ask the configured detection engines to determine if the target file is a malware or not.
If it is a malware, the operation is blocked and (if configured), the file is moved to quarantine.

### Detection engine

This client detects malware samples based on the currently used [database](https://github.com/simbiota/database-releases).
The current implementation uses a single detector, the [TLSH](https://github.com/trendmicro/tlsh) detector that
1. calculates the TLSH hash of the scanned sample
2. compares it to every sample in its database
3. returns positive detection is the TLSH difference is bellow a threshold (default: 40)

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
- `simbiota` binary itself: `~2.5MB`
- used libraries (`~1.1MB`):
  - `libc-2.31.so`: 1455120
  - `libdl-2.31.so`: 14560
  - `libm-2.31.so`: 633000
  - `libpthread-2.31.so`: 160200
  - `ld-2.31.so`: 145352
- database:
  - our ARM database is corrently `4.4MB`
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

[website-shield]: https://img.shields.io/badge/-Website-informational.svg?logo=data:image/png;base64,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