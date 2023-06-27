![API Stability: Alpha](https://img.shields.io/badge/API%20Stability-Alpha-red)

# Noosphere CLI

The Noosphere CLI is a reference client and pedagogical tool to demonstrate
the principles of the Noosphere protocol and give interested users a
no-code, low-complexity tool to synchronize content through the Noosphere.


## Usage
### User perspective
```sh
# Create an identity
orb key create `whoami`

# make a directory for your sphere.
mkdir my-sphere
cd my-sphere

# Generate your personal sphere.
orb sphere create --owner-key `whoami`

# ..now make edits ..

# see the status of files in your directory
orb status

# persist changes to the sphere
orb save

# join a gateway, after you've set this identity as the counterpart
orb config set gateway-url <gatewayurl>

# sync your changes with the upstream gateway
orb sync

# sync data from a different sphere. Note, you'll need to
# `orb auth add <did>` from that other sphere.
orb sphere join <their DID> --local-key `whoami`

# then follow the onscreen instructions.
```

### Gateway perspective
Note: The name `mygateway` below isn't special. Just a chosen name.

```sh
# Create an identity
orb key create mygateway

# make a directory for your sphere.
mkdir my-sphere
cd my-sphere

# Generate your personal sphere.
orb sphere create --owner-key mygateway

# Pair this with a user identity
orb config set counterpart <DID from the user>
```
