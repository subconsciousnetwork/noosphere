# Noosphere Name System

The Noosphere Name System (NNS) is a distributed network, similar to [DNS], that resolves a sphere's identity to the latest address of a revision of the sphere.

## :warning: Status: Experimental :warning:

Noosphere is under active development and we are still working through some challenges, and anything documented here can change without warning during this time. This specification will be used in the future to support new clients using the Noosphere Name System for Noosphere, or possibly other related data, but for now should be considered internal documentation.

## Terms

* **Spheres** are a core structure in Noosphere representing an entire "workspace" or "notebook", containing many documents, with change history represented as a series of revisions, not dissimilar from Git. A sphere's capabilities are managed by a public/private [Ed25519] encryption key. The sphere's unique identity is represented and publically referenced as a [DID]. 
* **Decentralized Identifiers** ([DID]s) are public keys of a public/private key pair represented as a text string. The DIDs used in NNS are all derived from [Ed25519] encryption keys.
  * Example representation: `did:key:z6MkoE19WHXJzpLqkxbGP7uXdJX38sWZNUWwyjcuCmjhPpUP`
* **User Controlled Authorization Networks** ([UCAN]s) are an extension of JSON Web Tokens ([JWT], [RFC7519]), designed to enable ways of authorizing offline-first apps and distributed systems. Represented as JSON data, base64 encoded as a string.
* **[Multiaddr]s** is an extensible format describing a network address through multiple layers and protocols.
  * Example representation for a machine at IP address 10.20.30.1 with TCP port 20000: `/ip4/10.20.30.1/tcp/20000`
* **[PeerId]s** are [multihash]ed public keys. In NNS, PeerIds are derived from keys unrelated to spheres, and used to identify and communicate with DHT nodes in the network.
* **Content Identifiers** ([CID]s) are [multihash]ed [content addresses](https://docs.ipfs.tech/concepts/content-addressing/), where content can be identified by an immutable address. CIDs used in NNS are all CIDv1 hashed via [BLAKE2b256].

## Overview

A sphere in Noosphere contains internal data for an address book, where other sphere's identities ([DID]s) can be stored. DIDs are hardly memorizable, so they're stored with a name, like "Alice". When Bob wants to reference Alice's notes on dogs from his sphere in subtext, it'd look like `"@Alice/dogs"`. The "Alice" entry in the address book maps to a DID, like `did:key:z6MkoE19WHXJzpLqkxbGP7uXdJX38sWZNUWwyjcuCmjhPpUP`, but a [CID] address is needed in order to fetch, ideally the latest, contents from Alice's sphere. This is where the Noosphere Name System comes in.

Bob's gateway connects to the NNS, asking peers if anyone has an address for Alice's key. The address can be found from the network, and using [UCAN], can be validated that it was Alice publishing it from Alice's gateway, or maybe even from another peer on the network that has no idea who Alice is. Once the address is retrieved, Alice's sphere's content can be fetched and remixed.

Later, Bob wants Alice's latest content. Because these addresses are content-addressed CIDs, they're immutable, meaning everytime a sphere's content changes, it has a new address. Bob's gateway connects to the NNS again, looking for a newer address for Alice's sphere, comparing the UCAN timestamps from previous and current results for Alice's sphere.

Additionally, Bob's gateway is propagating Bob's sphere's latest revision address when the sphere changes for others to find.

## Network Spec

The NNS network adheres to [libp2p]'s [Kademlia DHT Spec](https://github.com/libp2p/specs/blob/master/kad-dht/README.md)\*, and defines a set of expected values, validations, and recommended configurations.

Nodes in the libp2p DHT are identified by [PeerId]. These **SHOULD** be generated from [Ed25519] keys.

\* *Technically, libp2p Kademlia DHT specifies that keys in the network should be CIDs. NNS uses DIDs as keys.*

### Protocols

The DHT nodes **MUST** implement the libp2p [Identify] spec.

### DHT Methods 

All methods in the underlying spec **MUST** be implemented, with cavaets below.

The network uses Value Records (using `GET_VALUE`, `PUT_VALUE` DHT methods) where the key **SHOULD** be a sphere's public [Ed25519] key encoded as a [DID] UTF-8 string, and the corresponding value **SHOULD** be a [UCAN] token encoded as a [JWT] string.

* `PUT_VALUE` methods **SHOULD** reject malformed UCAN tokens or UCAN tokens that do not pass Validation](#validation).
* `PUT_VALUE` and `GET_VALUE` methods with non-DID keys **SHOULD** be ignored.
* `ADD_PROVIDER` and `GET_PROVIDERS` methods are not currently used and **MAY** be fulfilled.
* `FIND_NODE` and `PING` **MUST** passthrough from the underlying spec.

### UCAN SpherePublish Token

The value stored in the DHT records is a [UCAN] encoded token. That token represents the signing key has authority to publish a new record for an identity in the NNS network. This SpherePublish token requires several UCAN fields. [CID]s used in this token **SHOULD** use version v1 with [BLAKE2b256] hash. The **Audience** field is the identity ([DID]) of the sphere this record maps, and the **Attenuation** **MUST** have a `"sphere/publish"` capability for the **Audience** sphere. There **MUST** be a **Fact** containing a `"link"` field with the **Audience** sphere's revision address as a [CID].

An example of a SpherePublish token for sphere `did:key:z6MkkVf..`, issued by `did:key:z6MkoE1..` with `"sphere/publish"` capabilities for `did:key:z6Mkkvf..`, pointing to content at `bafy2bzacec..`:

```json
{
  "iss": "did:key:z6MkoE19WHXJzpLqkxbGP7uXdJX38sWZNUWwyjcuCmjhPpUP",
  "aud": "did:key:z6MkkVfktAC5rVNRmmTjkKPapT3bAyVkYH8ZVCF1UBNUfazp",
  "att": [{
    "with": "sphere:did:key:z6MkkVfktAC5rVNRmmTjkKPapT3bAyVkYH8ZVCF1UBNUfazp",
    "can": "sphere/publish"
  }],
  "prf": [
    /* NEED TO ADD PROOFS */
  ],
  "fct": [{
    "link": "bafy2bzacec4p5h37mjk2n6qi6zukwyzkruebvwdzqpdxzutu4sgoiuhqwne72"
  }],
  "exp": 1668187823
}
```

### Validation

The [UCAN] SpherePublish token value from `GET_RECORD` and `PUT_RECORD` **SHOULD** be validated before stored and propagated. The token should contain all necessary information to self-authorize, validating the following:

* The **Issuer** and **Audience** are DID identifiers.
* **Expiration** is a defined Unix timestamp.
* The **Issuer** signed the token.
* The **Issuer** has **Attentuation** *with* **Audience** and *can* `"sphere/publish"`.
* The authorization occurs between **Not before** if specified, and before **Expiration**.
* There's one **Fact** with an `"link"` field with a [CID] as the value.

## FAQ

* Why not use Provider Records in the DHT?
  * We are strongly considering this ([#124](https://github.com/subconsciousnetwork/noosphere/issues/124)).
* Why not use [IPNS]?
  * We don't want to directly depend on IPNS.
  * We support one-to-many relationships with users to spheres. 
  * IPNS as implemented in Kubo/go-ipfs requires a 1:1 relationship between Kubo instance and IPNS public key
  * IPNS is [currently buckling under critical threshold](https://github.com/ipfs/kubo/issues/3860) of poor-quality peers
* Could the value of the DHT records be [CID]s pointing to a [JWT] instead of a [JWT] to reduce record size?
  * The top-level record is likely to change frequently, but its associated proofs are unlikely to change frequently. By making the record a [JWT], we can perform some validation immediately on the more dynamic data, and leverage caching for the seldom-changing proofs as [CID]s.

[JWT]: https://jwt.io/
[RFC7519]: https://www.rfc-editor.org/rfc/rfc7519
[Ed25519]: https://en.wikipedia.org/wiki/Ed25519
[DID]: https://www.w3.org/TR/did-core/
[DNS]: https://en.wikipedia.org/wiki/Domain_Name_System
[UCAN]: https://ucan.xyz/
[Multiaddr]: https://multiformats.io/multiaddr/
[libp2p]: https://libp2p.io
[CID]: https://github.com/multiformats/cid
[PeerId]: https://docs.libp2p.io/concepts/fundamentals/peers/
[Identify]: https://github.com/libp2p/specs/tree/master/identify
[multihash]: https://multiformats.io/multihash/
[BLAKE2b256]: https://www.blake2.net/
[IPNS]: https://docs.ipfs.tech/concepts/ipns/
