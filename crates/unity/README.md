# unipeek-unity

Read Unity asset packages without a GPU. This is the parsing half of
[`unipeek`](https://crates.io/crates/unipeek):

- stream a `.unitypackage` (gzip tar) once and index its assets, flagging hostile paths, duplicates
  and truncation;
- open zips, folders and single FBX files the same way;
- read Unity YAML, `.meta` and `.mat` files, and recognise lilToon, Poiyomi, MToon and built-in
  shaders;
- choose the main prefab, follow nested prefabs and bind every FBX material slot to its textures;
- write a report with a status (ok, degraded, unsupported, error) and the GUIDs missing from the
  package.

> **Alpha.** Before 1.0 a change of the minor version may break the API. `unipeek` and
> `unipeek-unity` are released together with the same version. See the
> [changelog](https://github.com/vivi-shiki/unipeek/blob/master/CHANGELOG.md).

```toml
[dependencies]
unipeek-unity = "0.1.0-alpha.2"
```

If you also render thumbnails, depend on `unipeek` instead; it re-exports this crate as
`unipeek::unity`.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT)
at your option.
