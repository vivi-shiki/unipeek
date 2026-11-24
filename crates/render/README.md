# unipeek

Preview Unity asset packages: read a `.unitypackage`, zip, folder or FBX, work out which prefab and
materials to show, and render a toon-style thumbnail on a headless wgpu device.

> **Alpha.** Thumbnails, reports, the `serve` protocol and the command line work. The interactive
> viewport and cross-package dependencies are not there yet. Before 1.0 a change of the minor version
> may break the API. See the [changelog](https://github.com/vivi-shiki/unipeek/blob/master/CHANGELOG.md).

```toml
[dependencies]
# Turn off the default `cli` feature when you only link the library.
unipeek = { version = "0.1.0-alpha.2", default-features = false }
```

`unipeek` re-exports the GPU-free parser [`unipeek-unity`](https://crates.io/crates/unipeek-unity)
as `unipeek::unity`, so one dependency is enough.

## Command line

```
cargo install unipeek --version 0.1.0-alpha.2
unipeek info  <input> [--json]
unipeek thumb <input> -o thumb.webp --size 512 --view 3q
unipeek serve
```

`<input>` is a `.unitypackage`, `.zip`, folder or `.fbx`, optionally followed by `#<path inside>`.
`serve` reads one JSON request per line on stdin and answers on stdout, so a host can run it in a
child process and survive a GPU driver crash.

## Docs

- [Usage guide](https://github.com/vivi-shiki/unipeek/blob/master/docs/USAGE.md) (Chinese): how a
  host calls the library, the `serve` protocol and the command line.
- [Roadmap](https://github.com/vivi-shiki/unipeek/blob/master/docs/ROADMAP.md) (Chinese).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.
