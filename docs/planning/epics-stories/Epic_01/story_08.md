# Story 08: Test Infrastructure

## Description

Set up test infrastructure using Kind (Kubernetes in Docker) for integration testing. This replaces Docker Compose and provides isolated test environments with Kubernetes-native service discovery.

## Acceptance Criteria

- [ ] Kind cluster configuration for test infrastructure
- [ ] Kubernetes manifests for PostgreSQL (namespace, PVC, deployment, service)
- [ ] Setup/teardown scripts matching DCops pattern (`dev-up`, `dev-down`)
- [ ] Tiltfile for local development with live reloading
- [ ] Connection string resolution (works from host and inside cluster)
- [ ] Test helpers module for integration tests
- [ ] All examples work with test infrastructure
- [ ] All tests pass with test infrastructure

## Technical Details

- Kind cluster configuration (`kind-config.yaml`)
- Kubernetes manifests using kustomize
- PostgreSQL deployment via Tilt (namespace and PVC created during cluster setup)
- Connection string script detects environment (host vs. cluster)
- Tilt port-forwards PostgreSQL to localhost:5432
- Test helpers use `TestDatabase` for connection management

## Dependencies

- Story 02: Integrate may_postgres as Database Client
- Story 03: Implement LifeExecutor Trait
- Story 07: Connection Health Checks

## Notes

- Replaces Docker Compose setup
- Follows DCops pattern for consistency
- Enables integration tests with real PostgreSQL
- Tilt provides live reloading for development
- Foundation for all future integration tests
