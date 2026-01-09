# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## Added

## Changed

## Fixed

## [0.17.0] - 2026-01-08

## Added

## Changed

- The update mechanism has been rewritten to optimize for speed by increasing parallelism.

## Fixed

- `bundled_font` feature should now work correctly.
- The time to check for updates should be far shorter in cases where there isn't an update.

## [0.16.0] - 2024-12-28

## Added

- `VELOREN_ASSETS_OVERRIDE` can now be set from the airshipper UI.

## Changed

- Most dependencies were updated.

## Fixed

- Ping refreshes no longer reset the selected sorting order.
- Unneeded images will be removed from cache.

## [0.15.0] - 2024-10-07

## Added

- Integrate server queries to show player counts, server version information, battlemode and better ping.

## Fixed

- Installing the game via the CLI works once again.

## [0.14.0] - 2024-09-12

## Changed

- further downloads will be made from Gitlab via installer.exe rather than .msi file

## [0.13.0] - 2024-08-31

## Fixed

- Airshipper will no longer wipe your veloren screenshots

## [0.12.0] - 2024-08-28

## Added
- Partial Downloads for Airshipper client

## Changed
- Updated server dependencies, switched to axum/sqlx

## Fixed

## [0.11.0] - 2024-02-14

## Added
- Airshipper will now query voxygen for WGPU backend support to configure it

## Changed
- Updated iced to 0.8.0

## Fixed


## [0.10.0] - 2022-11-29

## Added
- Server Browser now can show discord/youtube/reddit/website/email information

## Fixed
- Server Browser can now connect to servers with non-default port

## [0.9.0] - 2022-09-16

## Added
- Server Browser
- MacOS M1/M2 support (aarch64)

## Changed

## Fixed

## [0.8.0] - 2022-09-02

## Added
- complete redesign of the UI
- added UI for channels
- added announcements and update notification

## [0.7.0] - 2022-03-10

## Added
- pretty print markdown changelog
- support for multiple channels
- better architecture support
- specify env variables for Voxygen
- added support for github releases as distribution channel

## Fixed
- switched to 64 bit ids, as 32 bit ids where running out

## [0.6.0] - 2021-08-31

## Added
- select the airshipper backend server between production/staging/test
- add a new settings window to adjust values and start veloren with trace level

## Changed
- update iced

## [0.5.0] - 2021-07-13

## Added
- add troubleshooting guide
- create server image tagged with branch or tag automatically

## Changed
- serve veloren locally (removes s3 support) #147

## Fixed
- Gitlab Pipeline Event model
- CI not running on forks
- quote paths in msiexec call
- validate response status before download
- only use one rustfmt.toml
- leave one artifact per os on prune
- reduced minimum window size
- check status of webhook response

## [0.4.2] - 2020-12-16

## Changed

- Trim changelog and link to it [#106](https://github.com/Songtronix/Airshipper/pull/106)
- add compatibility shortcut to access cli only mode easier [#106](https://github.com/Songtronix/Airshipper/pull/106)

## Fixed

- Improved font rendering [#104](https://github.com/Songtronix/Airshipper/pull/104)
- Fix os error 50 (Veloren can't be started) [#106](https://github.com/Songtronix/Airshipper/pull/106)
- missing glibc [#111](https://github.com/Songtronix/Airshipper/pull/111)

## [0.4.1] - 2020-11-27

## Changed

- Encountering an error while starting Veloren no longer crashes Airshipper [#94](https://github.com/Songtronix/Airshipper/pull/94)
- Fallback to terminal incase the GUI fails [#97](https://github.com/Songtronix/Airshipper/pull/97)

## Fixed

- AMD Integrated Graphics not displaying text [#97](https://github.com/Songtronix/Airshipper/pull/97)
- News not updating [#97](https://github.com/Songtronix/Airshipper/pull/97)
- Download stopping at 24% due to an error [#97](https://github.com/Songtronix/Airshipper/pull/97)

## [0.4.0] - 2020-08-29

## Added

- bundle ssl for better cross distro compatibility [#67](https://github.com/Songtronix/Airshipper/pull/67)
- embed assets for easier distribution [#67](https://github.com/Songtronix/Airshipper/pull/67)
- use same font as Veloren [#67](https://github.com/Songtronix/Airshipper/pull/67)
- option to retry if download/install failed [#78](https://github.com/Songtronix/Airshipper/pull/78)
- offline support [#87](https://github.com/Songtronix/Airshipper/pull/87)

## Changed

- place screenshots, logs, game saves in profile [#74](https://github.com/Songtronix/Airshipper/pull/74)
- do not close Veloren if airshipper gets closed [#77](https://github.com/Songtronix/Airshipper/pull/77)
- prune log file automatically [87](https://github.com/Songtronix/Airshipper/pull/87)

## Removed

- extra terminal window [#67](https://github.com/Songtronix/Airshipper/pull/67)

## [0.3.2] - 2020-03-21

## Changed

- fix: update news when remote is newer [#40](https://github.com/Songtronix/Airshipper/pull/40)
- prefer dedicated gpu [#41](https://github.com/Songtronix/Airshipper/pull/41)
- log crashes in file [#45](https://github.com/Songtronix/Airshipper/pull/45)
- updated all dependencies [#46](https://github.com/Songtronix/Airshipper/pull/46)
- fix: avoid unneeded warning about corrupted version [#47](https://github.com/Songtronix/Airshipper/pull/47)

## [0.3.1] - 2020-03-09

## Changed

- Fixes critical issues for new users trying out airshipper. [#38](https://github.com/Songtronix/Airshipper/issues/38)

## [0.3.0] - 2020-02-23

### Added

- Airshipper updates itself on windows [#9](https://github.com/Songtronix/Airshipper/issues/9)
- Changelog [#26](https://github.com/Songtronix/Airshipper/issues/26)

### Changed

- remove outdated assets automatically [#28](https://github.com/Songtronix/Airshipper/issues/28)
- Updated iced to latest version [#31](https://github.com/Songtronix/Airshipper/issues/31)
- include platform in request header [#34](https://github.com/Songtronix/Airshipper/issues/34)

## [0.2.1] - 2020-02-06

### Changed

- fixed `VCRUNTIME140_1.dll` missing [#16](https://github.com/Songtronix/Airshipper/issues/16)
- Linux builds are zips but file ending is .tar.gz [#14](https://github.com/Songtronix/Airshipper/issues/14)
- airshipper does not display changelog and news [#13](https://github.com/Songtronix/Airshipper/issues/13)
- made GUI optional [#12](https://github.com/Songtronix/Airshipper/issues/12)

## [0.2.0] - 2020-02-02

### Added

- Added GUI

[unreleased]: https://gitlab.com/veloren/airshipper/-/compare?from=v0.17.0&to=master
[0.2.0]: https://github.com/veloren/Airshipper/releases/tag/v0.2.0
[0.2.1]: https://github.com/veloren/Airshipper/releases/tag/v0.2.1
[0.3.0]: https://github.com/veloren/Airshipper/releases/tag/v0.3.0
[0.3.1]: https://github.com/veloren/Airshipper/releases/tag/v0.3.1
[0.3.2]: https://github.com/veloren/Airshipper/releases/tag/v0.3.2
[0.4.0]: https://github.com/veloren/Airshipper/releases/tag/v0.4.0
[0.4.1]: https://github.com/veloren/Airshipper/releases/tag/v0.4.1
[0.4.2]: https://github.com/veloren/Airshipper/releases/tag/v0.4.2
[0.5.0]: https://github.com/veloren/Airshipper/releases/tag/v0.5.0
[0.6.0]: https://github.com/veloren/Airshipper/releases/tag/v0.6.0
[0.7.0]: https://github.com/veloren/Airshipper/releases/tag/v0.7.0
[0.8.0]: https://github.com/veloren/Airshipper/releases/tag/v0.8.0
[0.9.0]: https://github.com/veloren/Airshipper/releases/tag/v0.9.0
[0.10.0]: https://github.com/veloren/Airshipper/releases/tag/v0.10.0
[0.11.0]: https://github.com/veloren/Airshipper/releases/tag/v0.11.0
[0.12.0]: https://github.com/veloren/Airshipper/releases/tag/v0.12.0
[0.13.0]: https://github.com/veloren/Airshipper/releases/tag/v0.13.0
[0.14.0]: https://github.com/veloren/Airshipper/releases/tag/v0.14.0
[0.15.0]: https://github.com/veloren/Airshipper/releases/tag/v0.15.0
[0.16.0]: https://github.com/veloren/Airshipper/releases/tag/v0.16.0
[0.17.0]: https://gitlab.com/veloren/airshipper/-/tags/v0.17.0
