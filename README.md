![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)
[![Discord](https://img.shields.io/discord/1003419732516552724.svg?logo=discord&colorB=7289DA)](https://discord.gg/HmHypb6DCj)

# Noosphere

> Noosphere (noun):
> 1. Planetary consciousness. A hypothetical new evolutionary phenomena rising out of the biosphere.
> 2. A protocol for thought.

This repository contains documentation and specifications for the Noosphere protocol. Noosphere, like its namesake, is a worldwide medium for thinking together. We like to think of it as a protocol for thought.

Noosphere is the foundational protocol that the Subconscious app builds upon to enable an open-ended, permissionless multiplayer experience. The documentation and specifications in this repository are intended to enable others to contribute to our efforts, and also to build clients and deploy infrastructure that interoperates over Noosphere.

See [explainer](/design/explainer.md).

## Status: discovery

Our ambition is to build a new kind of web, but we have only begun to discover what that means. Our work is rapidly advancing but still in-progress, and we need your help to drive it forward!

Check out our [Roadmap][roadmap] see where we are headed.

Follow along with the daily development process on the [Noosphere kanban][noosphere-kanban].

## Project layout

The [`rust`][rust] folder contains the core implementation of the Noosphere protocol as well as convenience abstractions and a reference client and server. Most crates can be compiled for native targets and/or WASM targets as desired. In time, we intend that JavaScript-specific packages will be maintained in this repository, backed the core Rust implementation compiled to WASM.

The [`design`][design] folder contains documents describing Noosphere data structures and protocols in generalized terms. Our aspiration is to document the protocol sufficiently that other implementations can be built without having to dissect and analyze our code to do it.

## License

This project is dual licensed under MIT and Apache-2.0.

MIT: https://www.opensource.org/licenses/mit  
Apache-2.0: https://www.apache.org/licenses/license-2.0

[rust]: https://github.com/subconsciousnetwork/noosphere/tree/main/rust
[design]: https://github.com/subconsciousnetwork/noosphere/tree/main/design/
[roadmap]: https://github.com/orgs/subconsciousnetwork/projects/1/views/4
[noosphere-kanban]: https://github.com/orgs/subconsciousnetwork/projects/1/views/8

