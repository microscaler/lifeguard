# Story 02: Table Partitioning Support

## Description

Support PostgreSQL table partitioning in LifeModel. Partitioned tables should be transparent to application code.

## Acceptance Criteria

- [ ] Partitioned tables can be queried via LifeModel
- [ ] Partition pruning works (queries only hit relevant partitions)
- [ ] INSERT operations route to correct partition
- [ ] LifeModel handles partition metadata
- [ ] Unit tests demonstrate partitioned table usage

## Technical Details

- PostgreSQL partitioning: RANGE, LIST, HASH
- Partition pruning: automatic (PostgreSQL handles)
- INSERT routing: automatic (PostgreSQL handles)
- LifeModel: treat partitioned table as regular table
- Support: `CREATE TABLE ... PARTITION BY`

## Dependencies

- Epic 02: ORM Core (LifeModel)

## Notes

- Partitioning is transparent to application code
- Useful for large tables (time-series, logs)
- Consider adding partition management helpers

