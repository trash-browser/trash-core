# gpui-dotnet

Experimental .NET bindings for GPUI.

The Rust crate builds a native `gpui_dotnet` library with a C ABI. The C#
wrapper in `dotnet/Gpui.DotNet` uses P/Invoke to call that library.

## Build

From the repository root:

```sh
cargo build -p gpui_dotnet
```

Then build/run the example:

```sh
dotnet run --project crates/gpui_dotnet/examples/HelloWorld/HelloWorld.csproj
```

The example project copies `target/debug/gpui_dotnet.dll` into its output
directory after build.

## Current API

The current binding mirrors the first Python port: `Div`, text children, a
subset of GPUI style helpers, color helpers, and opening a single window.
