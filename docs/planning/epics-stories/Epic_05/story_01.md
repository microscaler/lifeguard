# Story 01: LifeReflector - Leader-Elected Raft System

## Description

Implement the leader election mechanism for LifeReflector using a Raft-style consensus algorithm. Only the leader should actively process PostgreSQL LISTEN/NOTIFY events.

## Acceptance Criteria

- [ ] Raft leader election implemented
- [ ] Only leader processes LISTEN/NOTIFY events
- [ ] Leader failover works (new leader elected on failure)
- [ ] Multiple LifeReflector instances can run (only one active)
- [ ] Leader election is fast (< 1 second)
- [ ] Unit tests demonstrate leader election and failover

## Technical Details

- Use Raft consensus algorithm (simplified version)
- Leader election:
  - Nodes communicate via network
  - Majority vote required for leader
  - Heartbeat to detect leader failure
- Store leader state in Redis or PostgreSQL
- Leader processes LISTEN/NOTIFY, followers standby

## Dependencies

- Epic 04: v1 Release (must be complete)
- Redis (external service)

## Notes

- This is critical for cache coherence
- Raft ensures only one active reflector
- Consider using existing Raft library (e.g., `raft-rs`)

