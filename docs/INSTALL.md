# Table of Contents

- [Table of Contents](#table-of-contents)
- [How to build and install Eruption from source](#how-to-build-and-install-eruption-from-source)
    - [Install build dependencies](#install-build-dependencies)
      - [On Arch-based distros](#on-arch-based-distros)
      - [On Fedora-based distros](#on-fedora-based-distros)
      - [On Debian-based distros](#on-debian-based-distros)
    - [Clone the project and build the release binaries](#clone-the-project-and-build-the-release-binaries)
    - [Install Eruption](#install-eruption)
    - [Run Eruption](#run-eruption)

# How to build and install Eruption from source

To build Eruption from source you need to have `git` and `rust` installed, and you need to install the build
dependencies of Eruption as well. You need at least the current `stable` release of `rust` (MSRV: `1.58`).
You probably may want to use [https://rustup.rs/](https://rustup.rs/).

### Install build dependencies

#### On Arch-based distros

```shell
sudo pacman -Sy libevdev hidapi systemd-libs dbus libpulse lua lua-socket gtksourceview4
sudo pacman -Sy xorg-server-devel libxrandr gtk3
```

#### On Fedora-based distros

```shell
sudo dnf install systemd dbus hidapi libevdev lua gtksourceview4 lua-socket-compat
sudo dnf install systemd-devel dbus-devel hidapi-devel libevdev-devel libusbx-devel \
  pulseaudio-libs-devel lua-devel libX11-devel libXrandr-devel gtk3-devel gtksourceview4-devel
```

#### On Debian-based distros

```shell
sudo apt install libusb-1.0-0-dev libhidapi-dev libevdev-dev libudev-dev libdbus-1-dev \
  libpulse-dev lua liblua-5.4-dev libx11-dev libxrandr-dev libgtk-3-dev libgdk-pixbuf2.0-dev \
  libatk1.0-dev libpango1.0-dev libcairo2-dev libgtksourceview-4.0-dev
```

### Clone the project and build the release binaries

```shell
git clone https://github.com/X3n0m0rph59/eruption.git
cd eruption
make
```

### Install Eruption

```shell
sudo make install
```

### Run Eruption

To activate Eruption now, manually start the daemons with the following command:

```shell
make start
```

Finally, if you want to use one of the audio visualizer profiles, then please select an audio device monitor e.g.
using `pavucontrol`.

Switch to a profile that utilizes the audio API of Eruption:
```shell
eruptionctl switch profile spectrum-analyzer-swirl.profile
```

Then use `pavucontrol` to assign a monitor of an audio device to the Eruption audio grabber.

![audio-grabber pavucontrol](assets/screenshot-audio-grabber-pavucontrol.png)
> NOTE: You have to select a profile that makes use auf the audio grabber first, otherwise the
> `eruption-audio-proxy` will not open an audio device for recording, and therefore will not be listed
