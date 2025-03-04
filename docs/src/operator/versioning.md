# Versioning

We follow the [semver](https://semver.org/) standard for versioning.

The following is considered the node's public API, and will therefore be considered as breaking changes.

- RPC gRPC specification (note that this _excludes_ inter-component gRPC schemas).
- Node configuration options.
- Database schema changes.

We intend to include our OpenTelemetry trace specification in this once it standardizes.

We _will_ also call out non-breaking behvioral changes in our changelog and release notes.
