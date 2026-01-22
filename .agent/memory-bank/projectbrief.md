# Lifeguard Project Brief

## Overview
Lifeguard is a coroutine-driven database runtime for Rust, built from the ground up for the `may` coroutine runtime. It's a complete, production-grade ORM and data access platform that provides SeaORM-like functionality but is architected natively for coroutines.

## Key Features
- Coroutine-native PostgreSQL ORM
- ModelTrait implementation with comprehensive edge case coverage
- JSON support as a core feature (always enabled)
- Type-safe column access via Column enums
- Query builder with SeaQuery integration

## Current Status
- ModelTrait fully implemented with 25 passing tests
- Edge case coverage: 85%
- Test coverage: 75%
- JSON support: Core feature, always enabled
