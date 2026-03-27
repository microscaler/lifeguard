# Story 08: Pagination Helpers

## Description

Implement pagination helper methods that replicate SeaORM's `paginate()` and `paginate_and_count()` methods. These provide convenient pagination with automatic page calculation.

## Acceptance Criteria

- [ ] `find().paginate(pool, page_size)` - Returns paginator
- [ ] `find().paginate_and_count(pool, page_size)` - Returns paginator with total count
- [ ] Paginator provides: `fetch_page(page_num)`, `num_pages()`, `num_items()`
- [ ] Pagination works with all query builder filters
- [ ] Efficient count queries (avoid full table scans)
- [ ] Unit tests demonstrate pagination usage

## Technical Details

- Paginator struct:
  ```rust
  pub struct Paginator {
      query: QueryBuilder,
      page_size: usize,
      total: Option<usize>, // if paginate_and_count was used
  }
  
  impl Paginator {
      fn fetch_page(&self, page_num: usize) -> Result<Vec<LifeModel>>;
      fn num_pages(&self) -> usize;
      fn num_items(&self) -> Option<usize>;
  }
  ```
- `paginate()`: Uses LIMIT/OFFSET, doesn't count total
- `paginate_and_count()`: Executes COUNT query first, then fetches page
- Page numbers are 1-based (matches SeaORM)
- Efficient counting: use `SELECT COUNT(*) FROM (subquery)` for complex queries

## Dependencies

- Story 05: Type-Safe Query Builders

## Notes

- Pagination is essential for large datasets
- Should match SeaORM's pagination API
- Consider adding cursor-based pagination in future (for very large datasets)

