# dwm-forge Gentoo overlay

This overlay installs the patched dwm build through Portage's `savedconfig` support.

1. Place this repository at `/var/db/repos/dwm-forge-local` and create
   `/etc/portage/repos.conf/dwm-forge-local.conf`:

   ```ini
   [dwm-forge-local]
   location = /var/db/repos/dwm-forge-local
   auto-sync = no
   ```

2. Configure `/etc/portage/package.use/dwm` to enable the optional Xinerama
   support and saved configuration:

   ```text
   x11-wm/dwm xinerama savedconfig
   ```

3. Put the dwm configuration to preserve at
   `/etc/portage/savedconfig/x11-wm/dwm`. The release's verified `config.h` is
   synchronized to this path by the helper, so review or back up local changes
   before syncing.

4. Run `dwm-forge sync-gentoo`. It downloads the latest release artifacts,
   verifies `SHA256SUMS`, updates the saved config and Portage distfiles (including
   the stable distfile name expected by the ebuild), then regenerates the ebuild
   manifest and installs `x11-wm/dwm`.
