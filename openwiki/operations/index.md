# Files

- [Build, Tooling, CI and Release Operations](build-release-and-ci.md) - The operational surface of limedl — build prerequisites and rustflags, the xtask tooling, the mandatory pre-commit gate and its Windows blind spot, the CI job graph with nextest/coverage/sonar/supply-chain, and the tag-driven release pipeline including the static musl server artifacts.
- [Server Deployment and Packaging](server-deployment-and-packaging.md) - The committed deployment assets for the headless daemon — the hardened systemd unit and env file, the container entrypoint with its PUID/PGID convention, the from-source and prebuilt-binary Dockerfiles, the compose example and the multi-arch GHCR image.
