# Story 05: Performance Benchmarks

## Description

Establish performance benchmarks comparing Lifeguard to SeaORM and raw SQL. Benchmarks should demonstrate 2-5× improvement on hot paths.

## Acceptance Criteria

- [ ] Benchmark suite created
- [ ] Benchmarks compare: Lifeguard vs SeaORM vs raw SQL
- [ ] Benchmarks cover: simple queries, complex queries, connection pooling
- [ ] Results show 2-5× improvement over async ORMs
- [ ] Benchmark results documented
- [ ] Benchmarks are reproducible

## Technical Details

- Use `criterion` crate for benchmarking
- Benchmarks:
  - Simple SELECT (by primary key)
  - Complex SELECT (joins, filters)
  - INSERT operations
  - UPDATE operations
  - Connection pool performance
- Run on consistent hardware
- Document methodology

## Dependencies

- Epic 01-03: Core features must be complete

## Notes

- Benchmarks validate performance claims
- Should be run regularly (CI/CD)
- Consider adding benchmark comparisons to docs

