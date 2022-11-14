# Noosphere Explainer

_Last updated: July 2nd, 2022_

Noosphere is a massively-multiplayer knowledge graph. The technical pillars that Noosphere builds upon are:

- [Public key infrastructure](https://en.wikipedia.org/wiki/Public_key_infrastructure)
- [Content addressing](https://en.wikipedia.org/wiki/Content-addressable_storage)
- [Immutable data](https://en.wikipedia.org/wiki/Immutable_object)
- [P2P routing and discovery](https://en.wikipedia.org/wiki/Peer-to-peer)

Above this substructure, Noosphere gives users:

- Entry to a zero-trust, **decentralized network** of self-sovereign nodes
- **Human-readable names** for peers and their public content
- **Local-first authoring** and offline-available content with conflict-free synchronization
- A complete, space-efficient **revision history** for any content
- Coherence and **compatibility with the hypertext web**

You can think of it like a world-wide Wiki.

## How it feels…

Subconscious user Bob authors a note about cats, formatted as Subtext:

| _Bob's notebook · /cat-thoughts_            |
| ------------------------------------------- |
| I love cats.<br />I love every kind of cat. |

Notes in Bob's Subconscious notebook have a corresponding slug (e.g., _cat-thoughts_), that can be used as anchors for linking across notes. Later on when Bob authors another note on a related topic, they include a link to their first note:

| _Bob's notebook · /animal-notes_                               |
| -------------------------------------------------------------- |
| I have strongly felt **/cat-thoughts**<br />Dogs are just okay |

By following the link, Bob or another reader can navigate from one note to another note within Bob's local notebook.

One day Bob meets Alice and they get to talking about their shared interest in animals. Bob discovers that Alice also uses Subconscious. They exchange contact details, and soon Alice is able view an index of Bob's public notes:

| _Bob's notebook · Links_ |
| ------------------------ |
| /animal-notes            |
| /cat-thoughts            |
| /...                     |

After reading through Bob's cat-thoughts, Alice decides to reference it for later review. Alice opens up a local note about animals and links to Bob's note from there:

| _Alice's notebook · /awesome-animal-links_                                                                                      |
| ------------------------------------------------------------------------------------------------------------------------------- |
| Here are some cool **/zebra-facts**<br />I love that **/skateboarding-dogs** exist<br />Bob sure has some **@bob/cat–thoughts** |

By following the link **@bob/cat-thoughts**, Alice can navigate from a note in her local notebook to a note in Bob's notebook.

Alice and Bob do not need to give custody of their personal data or their identities to a third party for this link to work. Nor do they need to rely on a blockchain-based ledger.

## How it works…

Let's break down how a link like **@bob/cat–thoughts** works. Please note that what follows contains some simplification and shorthand; outbound links to detailed references have been included for the curious reader.

### Public key infrastructure

When Alice and Bob exchanged contact details in the story above, the exchanged data included [public keys][public key] (encoded as [DIDs][did] that represent their respective notebooks. Bob's public key was then recorded against a [pet name][petname] in Alice's notebook:

| _Alice's notebook · Names_                                            |
| --------------------------------------------------------------------- |
| mallory => `did:key:z6MktafZTREjJkvV5mfJxcLpNBoVPwDLhTuMg9ng7dY4zMAL` |
| bob => `did:key:z6MkffDZCkCTWreg8868fG1FGFogcJj5X6PY93pPcWDn9bob`     |

### Content addressing

As Alice updated her notebook to include Bob's name, a snapshot of her entire notebook at its latest state was recorded and condensed down to a short, unique ID: [a content ID, or CID][cid]. Such a snapshot and corresponding CID is recorded for every update to every user's notebook (including Bob's).

Similarly, when Bob updated their _animal-notes_ to include a link to _cat-thoughts_, a new snapshot of the note was recorded and a CID was computed for it. When Alice viewed the index of links in Bob's notebook, what she actually viewed was a mapping of note slugs to their CIDs:

| _Bob's notebook · Links_                  |
| ----------------------------------------- |
| /animal-notes => Cid(bafy2bza...3xcyge7s) |
| /cat-thoughts => Cid(bafy2bza...thqn3hrm) |
| /...                                      |

### Immutable data

Noosphere data is formatted in terms of [IPLD][ipld] and encoded in a low-overhead binary format called [DAG-CBOR][dag-cbor]. Even though a full snapshot is recorded with every revision, new storage space is only allocated for the delta between any two revisions. This strategy is similar to how Git efficiently stores the delta between any two sequential commits.

A data structure that we call a _Memo_ is used to pair open-ended header fields with a retained historical record of revisions to notebooks and their contents:

![Memo](images/Memo_1.png 'Memo')

The properties of immutable data structures allow content to be edited offline for an indefinite period of time and safely copied to replicas without the risk of conflicts at the convenience of the client.

### P2P routing and discovery

Every user who publishes to the network does so via a gateway server. The gateway represents the boundary edge of user sovereignty, and also gives the user a reliably available foothold in the Noosphere network. The owner of a notebook is also the owner of the gateway, and third-parties neither have or need direct access to it (even in managed hosting scenarios).

The owner of a notebook enables the gateway to publish the notebook to the network using a [UCAN][ucan]-based authorization scheme. UCANs establish a cryptographic chain of proof that allows anyone to verify in place that the gateway was authorized to perform actions such as signing content on the user's behalf (and without asking the user to share cryptographic keys with the gateway).

When the user updates their notebook, they replicate the revision deltas to the gateway over HTTP (as network availability allows), and also tell the gateway which CID represents the latest version of the notebook. The revision deltas are syndicated to the public network via [IPFS][ipfs]. Then, the gateway publishes the latest revision CID to a [DHT][dht] (see: [Noosphere Name System][noosphere-ns]) mapping it to the notebook's public key and pairing it with the UCAN proof chain.

### Putting it all together

After the gateway publishes an update to the DHT, it becomes possible for anyone who knows the public key of a notebook to discover the latest published revision of that notebook as a CID.

When that CID is known, it becomes possible for the entire notebook (or discrete parts of it) to be downloaded from IPFS and replicated to the local client of the reader.

Once the notebook is downloaded, any slug can be resolved to content using the slug-to-CID mapping recorded in the notebook's link index.

Here is Alice's notebook, visualized as simplified data structures; the content referenced by **@bob/cat-thoughts** is able to be resolved with a combination of metadata in Alice's notebook, and details published by Bob to the DHT:

![Noosphere](images/Noosphere_1.png 'Noosphere')

[public key]: https://en.wikipedia.org/wiki/Public-key_cryptography
[petname]: http://www.skyhunter.com/marcs/petnames/IntroPetNames.html
[did]: https://www.w3.org/TR/did-core/
[cid]: https://docs.ipfs.io/concepts/content-addressing/
[ipld]: https://ipld.io/
[ipfs]: https://ipfs.io/
[dag-cbor]: https://ipld.io/docs/codecs/known/dag-cbor/
[ucan]: https://ucan.xyz/
[dht]: https://en.wikipedia.org/wiki/Distributed_hash_table
[noosphere-ns]: name-system.md
