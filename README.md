# Trash Browser

## Build

The build helper downloads the matching CEF 150 distribution for Windows or
Linux, extracts it under `build/`, builds the React shell in `shell/`, sets
`CEF_PATH`, and invokes Cargo:

```sh
python build.py
```

To check without producing the final executable:

```sh
python build.py --check
```

Pass additional Cargo options after `--`, for example:

```sh
python build.py -- --release
```

On Windows, the script uses Ninja from `PATH` or from a Visual Studio
installation. If neither is available, install it with:

```powershell
winget install Ninja-build.Ninja
```

Run Cargo through the helper whenever CEF is not configured globally; the
`CEF_PATH` value is scoped to the Cargo process started by the script.

The single-file production shell is embedded in the Rust binary and served by
CEF through routes such as `miko://shell/#/new-tab` and
`miko://shell/#/settings`. The omnibox accepts `miko://#/settings` as a
shorthand.
