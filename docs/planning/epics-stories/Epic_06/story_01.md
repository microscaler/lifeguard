# Story 01: PostGIS Support

## Description

Add PostGIS support to LifeModel for geospatial data types. Support Point, Polygon, LineString, and other PostGIS geometry types.

## Acceptance Criteria

- [ ] PostGIS types supported: `Point`, `Polygon`, `LineString`, `MultiPoint`, etc.
- [ ] LifeModel can store/retrieve PostGIS geometries
- [ ] Spatial queries supported: `ST_Distance`, `ST_Within`, `ST_Intersects`, etc.
- [ ] Unit tests demonstrate PostGIS usage

## Technical Details

- Use `postgis` crate or custom serialization
- PostGIS types:
  - `Point(x, y)` → `POINT`
  - `Polygon` → `POLYGON`
  - `LineString` → `LINESTRING`
- Spatial queries: use SeaQuery or raw SQL
- Support: `ST_Distance`, `ST_Within`, `ST_Intersects`, `ST_Buffer`, etc.

## Dependencies

- Epic 02: ORM Core (LifeModel)
- PostGIS extension (PostgreSQL extension)

## Notes

- PostGIS is essential for logistics, sales territories, field service
- Consider adding spatial index support
- Document PostGIS-specific features

