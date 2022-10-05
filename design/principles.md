This document outlines values and motivations that have influenced the Noosphere protocol. We embrace growth and learning, so this document should be considered a living document.

## Purpose

Mission: unstoppable tools for thinking together.

Noosphere is a protocol for thought. Its goal is to offer an open, decentralized, shared commons for knowledge.

Noosphere builds on the foundation [content-addressing](https://web3.storage/docs/concepts/content-addressing/) and [IPLD data structures](https://ipld.io/), layering in:

- Naming, through a hyperlocal p2p petname system
- Change history, like a lightweight Git
- Security, through self-sovereign public key-based authorization ([UCAN](https://ucan.xyz/))

The result is a hypertext protocol for user-owned data that is open-ended like http, versioned like git, and decentralized through content addressing. Sort of like a worldwide wiki that any app can use as a backend.

More background:
- [Noosphere announcement](https://subconscious.substack.com/p/noosphere-a-protocol-for-thought)
- [Redecentralizing the web](https://subconscious.substack.com/p/redecentralization)

## Principles

These are some of the values that have inspired or guided aspects of Noosphere's design.

### Noosphere supports credible exit

> To avoid lock-in you need the ability for the user to credibly exit. ([cdixon](https://twitter.com/cdixon/status/1444457003439443973?s=20&t=-mwTiFugTSLJhvVuujKhpA))

> DNS is the unsung hero of web1. A mapping between network layer (domain name) and physical layer (IP), controlled fully by users, enabling them to keep centralized services in check with a credible ability to exit. ([cdixon](https://twitter.com/cdixon/status/1485323247755448331?s=20&t=M3gNBPIOU-QZHUg9jCyPng))

[Credible exit](https://subconscious.substack.com/p/credible-exit) means the ability to leave an app or service without losing access to what you've created. For us, credible exit is closely related to the value of user ownership. Users in Noosphere have autonomy over their data, and over their own privacy, and [legibility](https://subconscious.substack.com/p/soulbinding-like-a-state).

Noosphere aims to support three core kinds of exit at the protocol layer:

- Own your data: Spheres act as personal data backpacks. Your data is yours and you can take it with you between app and services.
- Own your identity: Noosphere's security is built on top of [self-sovereign](https://en.wikipedia.org/wiki/Self-sovereign_identity) authz primitives ([UCAN](https://ucan.xyz/)). You own and control your keys.
- Self-sovereign social graph: like email, you can take your address book with you. You can change apps and services while keeping your followers and the people you follow.

### Noosphere is evolvable

> "I argue that much of the Internet's success can be attributed to its users' ability to shape the network to meet their own objectives." (Abbate, 1999. Inventing the Internet)

> Only that which can change can continue. (James Carse)

Noosphere is designed for open-ended evolution, to be adapted and evolved in directions we can't yet imagine.

### Noosphere is permissionless

We aim to build protocols that provoke permissionless innovation—the ability to do new things without hitting coordination problems.

### Noosphere aims for decentralization...

> Evolution is the most decentralized thing that you can imagine. It is something that runs itself and is self-organizing at every level and at every scale. (Stewart Brand)

Decentralization makes ecosystems resilient and enables many of the other values we embrace, including evolvability, pluralism, user-ownership, and subsidiarity.

We value decentralization in protocol design, and avoid centralization. Centralization can be usefully defined as "the ability of a single entity or a small group of them to exclusively observe, capture, control, or extract rent from the operation or use of a function" ([IETF](https://www.ietf.org/archive/id/draft-nottingham-avoiding-internet-centralization-05.html), 2022).

#### ...and aims to mitigate downside where centralization emerges

[Centralization emerges organically in all evolving systems](https://subconscious.substack.com/p/centralization-is-inevitable). You can never fully escape it, but you can sometimes mitigate it.

Where centralization is likely to emerge, or where decentralized approaches are infeasible, we look for pragmatic ways that the protocol can mitigate downsides and uphold Noosphere's other values (credible exit, permissionlessness, etc). We are inspired by the way the internet created consortiums and other open goverance structures to distribute responsibility for infrastructure (such as DNS), that has strong centralizing tendencies.

### Noosphere is a commons

Following [Elinor Ostrom](https://subconscious.substack.com/p/wiki-as-a-commons), we think the most effective way to manage a commons is through locally-situated community. 

We value self-determination and local governance, beginning with individuals and small [Dunbar-scale communities](https://subconscious.substack.com/p/dunbar-scale-social) (the [Cozyweb](https://studio.ribbonfarm.com/p/the-extended-internet-universe)), before working our way up.

This principle is called "subsidiarity". Governance decisions are made from the bottom-up, at the lowest practical level. It is similar in spirit to the [principle of least authority](https://en.wikipedia.org/wiki/Principle_of_least_privilege). Subsidiarity makes space for pluralism and self-determination. We value it as an alternative to top-down, totalizing, and centralized modes of governance and moderation.

### Noosphere is for everybody

> Plurality is the condition of human action because we are all the same, that is, human, in such a way that nobody is ever the same as anyone else who ever lived, lives, or will live. (Hannah Arendt)

Pluralism is a core value for Noosphere. We want Noosphere to be accessible to anyone who wants to use it, and adaptable toward a plurality of uses. We value building technology that is accessible to everyone, regardless of gender identity and expression, sexual orientation, disability, personal appearance, body size, race, ethnicity, age, religion, nationality, or other characteristics.

### Design is navigating tradeoffs by values

Design is the practice of navigating [wicked problems](https://en.wikipedia.org/wiki/Wicked_problem)—problems for which there is no one optimal solution. The values that guide Noosphere's design aren't absolutes, but they do frame a design space that is meaningful for the particular goals of this protocol.

These values will not always be fully realizable, and will sometimes be in tension. When this happens, we embrace pragmatic trade-offs and aim for continual improvement. Like the IETF, we move forward through rough consensus and running code.

# Appendix

## Scope

The internet is a big tent. Noosphere hopes to be a big tent, too. We aim to keep this document focused on a small set of enabling principles. By maintining a minimal ideological footprint, we leave space for growth and pluralism.

## Prior art

Other principles documents to be inspired by:

- https://www.ietf.org/about/participate/tao/
- https://www.ietf.org/archive/id/draft-nottingham-avoiding-internet-centralization-05.html 
- https://www.rfc-editor.org/rfc/rfc1958.html 
- https://www.w3.org/DesignIssues/Principles 
- https://www.w3.org/TR/design-principles/ 
- https://www.w3.org/1998/02/Potential.html 