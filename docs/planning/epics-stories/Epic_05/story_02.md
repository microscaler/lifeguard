# Story 02: LifeReflector - PostgreSQL LISTEN/NOTIFY Integration

## Description

Implement PostgreSQL LISTEN/NOTIFY subscription in LifeReflector. The reflector should subscribe to database change events and process notifications.

## Acceptance Criteria

- [ ] LifeReflector subscribes to PostgreSQL LISTEN channels
- [ ] NOTIFY events are received and processed
- [ ] Notification format: `table_changes, '{"id": 42}'`
- [ ] LifeReflector handles notification parsing
- [ ] Multiple tables supported (one channel per table)
- [ ] Unit tests demonstrate LISTEN/NOTIFY integration

## Technical Details

- Use `may_postgres` LISTEN/NOTIFY support
- Subscribe to channels: `lifeguard_<table_name>_changes`
- Notification payload: JSON with changed row ID
- Process notifications asynchronously (coroutine-based)
- Handle connection failures and reconnection

## Dependencies

- Story 01: LifeReflector - Leader-Elected Raft System

## Notes

- LISTEN/NOTIFY is PostgreSQL's pub/sub mechanism
- LifeRecord should trigger NOTIFY on writes
- Consider batching notifications for performance

