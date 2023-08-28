# How to install Simbiota on a Raspberry Pi

Simbiota uses the [fanotify](https://man7.org/linux/man-pages/man7/fanotify.7.html) kernel API to monitor file system events.
In order to be able to block the execution of malware, it needs the `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` kernel config variable set to `y` (enabled).
Without this feature, Simbiota can only notify on a detection event and move the file to quarantine but at this point the potentially malicious code has already been executed.
To enable `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` feature in the kernel, it needs to be recompiled.

You can check whether `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` is enabled in your running kernel with the following commands:
```bash
modprobe configs
if zcat /proc/config.gz | grep -q CONFIG_FANOTIFY_ACCESS_PERMISSIONS=y;
then
    echo "FANOTIFY_ACCESS_PERMISSIONS are enabled, you can run Simbiota";
else
    echo "Kernel needs to be recompiled to support Simbiota";
fi
```

If you need to recompile the kernel, you can follow the instructions in [Preparing your device](#preparing-your-device) section.
Otherwise, if your device is ready, proceed to [installing `simbiota`](./README.md#install-the-released-package-raspberry-pi-arm64armv7):

## Preparing your device

- Rebuilding the kernel is needed to utilize `fanotify` kernel feature.

### 0 Preparing a working Raspberry Pi device

**If you already have a working device, you can skip to [Rebuilding the kernel](#1-rebuilding-the-kernel)**

You can either use [Raspberry Pi Imager](https://www.raspberrypi.com/software/) or perform the steps manually.
Here we only show the manual option as the Imager is well-guided.

1. Figure out whether your device supports 64-bit addresses or only legacy 32-bit addresses.
    ```bash
    arch=$(uname -m)
    if [[ ${arch} == "aarch64" ]];
    then
        echo "You are running an arm64, 64-bit kernel.";
    elif [[ ${arch} == "armv7l" ]];
    then
        echo "You are running an armv7l, 32-bit kernel";
    fi
    ```

1. Download the selected image from [the official site](https://www.raspberrypi.com/software/operating-systems/).
    Choose either `Desktop` and `Lite` edition. The `Desktop` version has a Graphical User Interface and many preinstalled utilities.

    ```bash
    wget -nH https://downloads.raspberrypi.org/raspios_lite_arm64/images/raspios_lite_arm64-2023-05-03/2023-05-03-raspios-bullseye-arm64-lite.img.xz
    wget -nH https://downloads.raspberrypi.org/raspios_lite_arm64/images/raspios_lite_arm64-2023-05-03/2023-05-03-raspios-bullseye-arm64-lite.img.xz.sha256

    sha256sum -c ./2023-05-03-raspios-bullseye-arm64-lite.img.xz.sha256
    ```

    VERIFY that the output of the previous command is `OK`.

1. Write the image to the SD card.

    ```bash
    # attach the SD card to your computer, BUT DON'T MOUNT IT
    # IMPORTANT! replace /dev/sda in the following command with the device path of the SD card you wish to place into your RPi device
    RPI_SD_CARD_DEVICE=/dev/sda
    xzcat ./2023-05-03-raspios-bullseye-arm64-lite.img.xz | dd of="${RPI_SD_CARD_DEVICE}" bs=4M status=progress
    sync    # write pages to SD card
    ```

1. Detach the SD card from your computer and boot up the device. Follow the on screen instructions if there are any until you reach a shell.

    ```console
    user@raspberrypi:~ $
    ```

1. Now power off the device and reattach the SD card to your computer. Then follow with the steps for [rebuilding the kernel](#1-rebuilding-the-kernel).

### 1 Rebuilding the kernel

You can either follow the guide on [the official site](https://www.raspberrypi.com/documentation/computers/linux_kernel.html) (recommended), or use our briefed version here.

#### arm64 (aarch64)

The following commands are for cross-compiling the kernel from an AMD64 (x86-64) machine to an `aarch64 (arm64)` RPi target.

For cross-compilation you need the GNU GCC Toolchain for which you probably have a package in your distribution:
- ubuntu/debian: `crossbuild-essential-arm64`

If not, you can download the somewhat official binary release from [ARM website](https://developer.arm.com/downloads/-/arm-gnu-toolchain-downloads).
Note that in this case you need to use `aarch64-none-linux-gnu-` instead of `aarch64-linux-gnu-` in the following commands.

```bash
git clone https://github.com/raspberrypi/linux
cd linux
KERNEL=kernel8
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- bcm2711_defconfig
sed -ri 's/# CONFIG_FANOTIFY_ACCESS_PERMISSIONS is not set/CONFIG_FANOTIFY_ACCESS_PERMISSIONS=y/' ./.config
# the following command will rebuild the kernel and may take a long time (~1-2h) with increased CPU usage
make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- Image modules dtbs
```

```bash
# IMPORTANT! replace /dev/sda in the following command with the device path of the SD card you wish to place into your RPi device
RPI_SD_CARD_DEVICE=/dev/sda

mount --mkdir ${RPI_SD_CARD_DEVICE}2 /mnt/rpi/
mount --mkdir ${RPI_SD_CARD_DEVICE}1 /mnt/rpi/boot

env PATH=$PATH make ARCH=arm64 CROSS_COMPILE=aarch64-linux-gnu- INSTALL_MOD_PATH=/mnt/rpi/ modules_install

cp arch/arm64/boot/Image /mnt/rpi/boot/$KERNEL-fanotify.img
cp arch/arm64/boot/dts/broadcom/*.dtb /mnt/rpi/boot/
cp arch/arm64/boot/dts/overlays/*.dtb* /mnt/rpi/boot/overlays/
cp arch/arm64/boot/dts/overlays/README /mnt/rpi/boot/overlays/

echo "kernel=$KERNEL-fanotify.img" >> /mnt/rpi/boot/config.txt

umount /mnt/rpi/boot
umount /mnt/rpi/
```

#### armv7l

The following commands are for cross-compiling the kernel from an `AMD64 (x86-64)` machine to an `armv7l` RPi target.

For cross-compilation you need the GNU GCC Toolchain for which you probably have a package in your distribution:
- ubuntu/debian: `crossbuild-essential-armhf`

If not, you can download the somewhat official binary release from [ARM website](https://developer.arm.com/downloads/-/arm-gnu-toolchain-downloads).
Note that in this case you need to use `arm-none-linux-gnueabihf` instead of `arm-linux-gnueabihf` in the following commands.

```bash
git clone https://github.com/raspberrypi/linux
cd linux
```

If you are using a Raspberry Pi 2, 3, 3+ and Zero 2 W, and Raspberry Pi Compute Modules 3 and 3+:
```bash
KERNEL=kernel7
make ARCH=arm CROSS_COMPILE=arm-linux-gnueabihf- bcm2709_defconfig
```
If you are using a Raspberry Pi 4 and 400, and Raspberry Pi Compute Module 4:
```bash
KERNEL=kernel7l
make ARCH=arm CROSS_COMPILE=arm-linux-gnueabihf- bcm2711_defconfig
```
If you are using a Raspberry Pi 1, Zero and Zero W, and Raspberry Pi Compute Module 1:
```bash
KERNEL=kernel
make ARCH=arm CROSS_COMPILE=arm-linux-gnueabihf- bcmrpi_defconfig
```

Continue for all device types:

```bash
sed -ri 's/# CONFIG_FANOTIFY_ACCESS_PERMISSIONS is not set/CONFIG_FANOTIFY_ACCESS_PERMISSIONS=y/' ./.config
# the following command will rebuild the kernel and may take a long time (~1-2h) with increased CPU usage
make ARCH=arm CROSS_COMPILE=arm-linux-gnueabihf- zImage modules dtbs
```

```bash
# IMPORTANT! replace /dev/sda in the following command with the device path of the SD card you wish to place into your RPi device
RPI_SD_CARD_DEVICE=/dev/sda

mount --mkdir ${RPI_SD_CARD_DEVICE}2 /mnt/rpi/
mount --mkdir ${RPI_SD_CARD_DEVICE}1 /mnt/rpi/boot

env PATH=$PATH make ARCH=arm CROSS_COMPILE=arm-linux-gnueabihf- INSTALL_MOD_PATH=/mnt/rpi/ modules_install

cp arch/arm/boot/zImage /mnt/rpi/boot/$KERNEL-fanotify.img
cp arch/arm/boot/dts/*.dtb /mnt/rpi/boot/
cp arch/arm/boot/dts/overlays/*.dtb* /mnt/rpi/boot/overlays/
cp arch/arm/boot/dts/overlays/README /mnt/rpi/boot/overlays/

echo "kernel=$KERNEL-fanotify.img" >> /mnt/rpi/boot/config.txt

umount /mnt/rpi/boot
umount /mnt/rpi/
```

### 2 verify that the config is set

Now your SD card is ready! Insert it to your device and boot up the new kernel.

Verify that you have `CONFIG_FANOTIFY_ACCESS_PERMISSIONS` enabled:
```bash
modprobe configs
if zcat /proc/config.gz | grep -q CONFIG_FANOTIFY_ACCESS_PERMISSIONS=y;
then
    echo "FANOTIFY_ACCESS_PERMISSIONS are enabled, you can run Simbiota";
else
    echo "Kernel needs to be recompiled to support Simbiota";
fi
```