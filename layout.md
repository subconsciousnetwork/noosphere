```sh
/sphere_root/
 ├── .sphere/
 │   ├── identity
 │   ├── version
 │   ├── storage/ # Storage folder distinguishes the root sphere
 │   │   └── ... # Implementation-specific e.g., Sled will have its own DB structure
 │   ├── content/
 │   │   ├── bafyabc...a123
 │   │   ├── bafyabc...b456
 │   │   ├── bafyabc...c789
 │   │   └── ...
 │   └── peers/
 │       ├── did:key:abc/
 │       │   ├── .sphere/
 │       │   │   ├── identity
 │       │   │   ├── version
 │       │   │   └── peers/ # One peer shares version w/ root sphere, other does not
 │       │   │       ├── did:key:npq -> ../../did:key:npq
 │       │   │       └── did:key:xyz/
 │       │   │           ├── .sphere/
 │       │   │           │   └── ...
 │       │   │           └── old-baz.subtext
 │       │   ├── their-foo.subtext
 │       │   ├── @same-version-peer -> ./.sphere/peers/did:key:npq
 │       │   └── @peers-version-peer -> ./.sphere/peers/did:key:xyz
 │       ├── did:key:npq/
 │       │   ├── .sphere/
 │       │   │   └── ...
 │       │   └── more.subtext
 │       └── did:key:xyz/
 │           ├── .sphere/
 │           │   └── ...
 │           ├── baz.subtext
 │           └── ...
 ├── foo.subtext
 ├── bar/
 │   └── baz.subtext
 ├── @my-peer/ -> ./.sphere/peers/did:key:abc
 └── @other-peer/ -> ./.sphere/peers/did:key:xyz
```

```
  ↪ storage/
    ↪ ...
  ↪ peers/
    ↪ did:key:abc
      ↪ .sphere/
        ↪ identity
        ↪ version
        ↪ peers/
          ↪ did:key:npq -> ../../did:key:npq
          ↪ did:key:xyz/
            ↪ .sphere/
      ↪ their-foo.subtext
      ↪ @peers-peer -> ./.sphere/peers/did:key:npq
    ↪ did:key:xyz
      ↪ .sphere/
        ↪ ...
      ↪ other-bar/
      ↪ baz.subtext
    ↪ did:key:npq
      ↪ .sphere/
        ↪ ...
      ↪ more.subtext
↪ foo.subtext
↪ bar/
  ↪ baz.subtext
↪ @my-peer/ -> ./.sphere/peers/did:key:abc
↪ @other-peer/ -> ./.sphere/peers/did:key:xyc


```

orb sync --depth 3
