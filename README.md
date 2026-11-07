<div align="center">

# bevy_puppet

**Cut-out puppets for Bevy: bone entities, layered animation, constraints, pixel-perfect placement.**

[![Build](https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_puppet/build.yml?branch=main&label=build&style=flat-square)](https://github.com/declaredlost/bevy_puppet/actions/workflows/build.yml)
[![Security](https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_puppet/security.yml?branch=main&label=security&style=flat-square)](https://github.com/declaredlost/bevy_puppet/actions/workflows/security.yml)

</div>

A puppet is a skeleton of bone entities, posed every frame by layered
animations and constraints, and drawn by whatever the game parents under its
bones. One system for every body plan (a person, a spider, a bat), told
apart only by their rigs; and made for pixel art: joints on whole pixels,
mirrored exactly, sprites pre-rotated with RotSprite where they turn.

> [!WARNING]
> **Early development.** The API may change at any time.

## Crates

| Crate | Description |
|---|---|
| [`bevy_puppet`](crates/bevy_puppet) | Puppets: rigs and clips as assets, bone entities, layered animation, constraints |
| [`bevy_puppet_macros`](crates/bevy_puppet_macros) | `bevy_puppet`'s label derives |

## Requirements

- **Rust + Cargo**: 1.99+, Edition 2024
- **Bevy**: 0.20

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup and guidelines, and
[CHANGELOG.md](CHANGELOG.md) for release notes.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

