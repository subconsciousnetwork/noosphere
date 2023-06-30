# Noosphere TypeScript

![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?label=License)
![Tests](https://img.shields.io/github/workflow/status/subconsciousnetwork/noosphere/Run%20test%20suite/main?label=Tests)

The Noosphere TypeScript workspace includes the sources of the following NPM packages:

- **[`@subconsciousnetwork/orb`](./packages/orb)**: an implementation of the Noosphere API
  suitable for web browsers, built around the Noosphere Rust packages (compiled
  as Web Assembly).
- **[`@subconsciousnetwork/sphere-viewer`](./packages/sphere-viewer)**: a simple web app
  that implements read-only access to Noosphere using `@subconsciousnetwork/orb` as a dependency.
- **[`@subconsciousnetwork/noosphere-guide`](./packages/noosphere-guide)**: an
  [11ty][eleventy]-based static website generator that produces the Noosphere
  documentation website.

## Environment Setup

### Rust

Although the packages in this workspace are intended for use in a JavaScript runtime,
our Rust crates must be compiled as part of the development workflow. So, before
you get started, [follow these instructions](/rust/README.md#environment-setup)
to set up your environment for compiling Noosphere Rust packages to
[Web Assembly][web-assembly].

### Node.js

You will need a stable release of Node.js and NPM to be installed and up to date
in order to build these packages. The most reliable way to ensure you have this is to [install NVM][install-nvm] and then run this command:

```sh
nvm install --lts
```

As an extra step, you may wish to make the versions of Node.js and NPM you just installed into the default ones available when you open your shell:

```sh
nvm alias default lts/hydrogen
```

### NPM Dependencies

With Node.js and NPM installed, run the following command from this workspace to install all the NPM dependencies needed by all the packages that are maintained here:

```sh
npm install
```

## Building

All build-related actions are performed by NPM scripts, and orchestrated under the hood using [`wireit`][wireit]. So, from this directory you can run the following command:

```sh
npm run build
```

And, if you followed the instructions above, all the NPM packages will be built,
including all of their dependencies (even the Rust packages will be compiled to
Web Assembly and linked to the appropriate places in this workspace).

To run TypeScript tests in headless Chrome:

```sh
npm run test
```

And, to start servers that make both [Sphere Viewer](./packages/sphere-viewer)
and the [Noosphere Guide](./packages/noosphere-guide) available in web browsers:

```sh
npm run serve
```

You can add `--watch` to any command and it will watch for file changes (including changes to Rust sources) and re-run the necessary steps based on the file changes detected. For example:

```sh
npm run serve --watch
```

If you only care about a single package, most commands will work if you run them from the package root (instead of this workspace).

[web-assembly]: https://webassembly.org/
[install-nvm]: https://github.com/nvm-sh/nvm/blob/master/README.md#installing-and-updating
[npm-scripts]: https://docs.npmjs.com/cli/v6/using-npm/scripts
[wireit]: https://github.com/google/wireit
[eleventy]: https://www.11ty.dev/
