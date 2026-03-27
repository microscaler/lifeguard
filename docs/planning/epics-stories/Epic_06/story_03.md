# Story 03: Triggers and Stored Procedures Integration

## Description

Enable calling PostgreSQL triggers and stored procedures from Lifeguard. Support both calling stored procedures and reacting to trigger events.

## Acceptance Criteria

- [ ] Stored procedures can be called via LifeExecutor
- [ ] Procedure parameters supported (IN, OUT, INOUT)
- [ ] Trigger events can be handled (via LISTEN/NOTIFY)
- [ ] Unit tests demonstrate stored procedure calls

## Technical Details

- Stored procedures: `CALL procedure_name($1, $2, ...)`
- Parameters: use `may_postgres` parameter binding
- Return values: handle OUT/INOUT parameters
- Triggers: use LISTEN/NOTIFY (LifeReflector integration)
- Support: `CREATE FUNCTION`, `CREATE TRIGGER`

## Dependencies

- Epic 01: Foundation (LifeExecutor)
- Epic 05: LifeReflector (for trigger events)

## Notes

- Stored procedures are useful for complex business logic
- Triggers enable event-driven architectures
- Consider adding procedure result mapping

