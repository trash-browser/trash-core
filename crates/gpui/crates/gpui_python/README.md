# gpui-python

Experimental Python bindings for GPUI.

## Development

Build an editable extension with:

```sh
python -m pip install maturin
python -m maturin develop
```

Run the example after building:

```sh
python examples/hello_world.py
```

The current API is intentionally small and focuses on GPUI's `div()` element
builder, basic styling, text children, and opening a single window.
