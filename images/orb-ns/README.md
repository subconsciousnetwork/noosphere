
```bash
cd noosphere/

# Build
podman build -t orb-ns:latest -f images/orb-ns/orb-ns.Dockerfile .

# Run
podman run -it -v ~/.noosphere:/home/dhtuser/.noosphere -t orb-ns:latest bootstrap-key 6666 6667
```

