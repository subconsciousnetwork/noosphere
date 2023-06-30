---
layout: layouts/docs.njk
---

# Introduction

Noosphere is a **protocol** for a new kind of **content web**:

- **Protocol**: Noosphere is a common set of rules that support open-ended
  applications to be built on top of them.
- **Content web**: Noosphere is like the web; you can use it to publish files
  and link to them across the network.

At a high level, Noosphere has two features that make it a _useful_ web
protocol:

- **Content space**: Noosphere provides a space for users to save arbitrary
  kinds of files against human-readable names, much like a filesystem; these can
  be published to the network.
- **Address book**: Noosphere users keep an address book that contains other
  users or programs that they follow, enabling human-readable links to content
  that may traverse nodes in the network.

In other words: **Noosphere enables [hyperlinks][wiki-hyperlinks].** Hyperlinks
in Noosphere typically look like this:

![Slashlink example](../_static/images/content/slashlink-example.svg)

## Principles

Noosphere aims to promote **decentralization** in the network by designing
around the following principles:

- **Simplicity**: Noosphere delivers simple primitives that _enable_ complex
  applications
- **Evolvability**: Noosphere may be adapted and used in ways we can't yet imagine
- **Subsidiarity**: Noosphere privileges governance at the level of close-knit
  social communities
- **Credible exit**: Users are always in control of both their identity and
  their data

We are building Noosphere for everyone, and we hope you will build it with us.
To pitch in and help shape the project, join our community on
[Discord][subconscious-discord] and/or get involved with our open source project
on [Github][noosphere-github]!

## Technical design

Noosphere is designed to be compatible with the hypertext web you know and love.
[URL][wiki-url]-style hyperlinks still work. And, content on Noosphere may be
delivered directly to web browsers that speak [HTTP][wiki-http].

Noosphere also introduces a set of new technical concepts that may be unfamiliar
to those who come from a traditional web development background. Refer to the
[technical design foundations](/docs/foundations) section for a more detailed
exploration of this topic.

<!-- The technical pillars that Noosphere builds upon include:

- [Public key infrastructure](https://en.wikipedia.org/wiki/Public_key_infrastructure)
- [Content addressing](https://en.wikipedia.org/wiki/Content-addressable_storage)
- [Immutable data](https://en.wikipedia.org/wiki/Immutable_object)
- [P2P routing and discovery](https://en.wikipedia.org/wiki/Peer-to-peer)
- [End-to-end encryption](https://en.wikipedia.org/wiki/End-to-end_encryption)

Above this substructure, Noosphere gives users:

- Entry to a zero-trust, **decentralized network** of self-sovereign nodes
- **Human-readable names** for peers and their public content
- **Local-first authoring** and offline-available content with conflict-free synchronization
- A complete, space-efficient **revision history** for any content
- Coherence and **compatibility with the hypertext web**
 -->

[subconscious-discord]: https://discord.gg/wyHPzGraBh
[noosphere-github]: https://github.com/subconsciousnetwork/noosphere
[wiki-hyperlinks]: https://en.wikipedia.org/wiki/Hyperlink
[wiki-url]: https://en.wikipedia.org/wiki/URL
[wiki-http]: https://en.wikipedia.org/wiki/HTTP
