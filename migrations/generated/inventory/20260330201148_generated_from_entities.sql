-- Migration: Generated from Lifeguard entities
-- Service: inventory
-- Version: 20260330201148
-- Generated: 2026-03-30 20:11:48 UTC

-- This migration was automatically generated from entity definitions.
-- DO NOT EDIT MANUALLY - regenerate from entities instead.

-- Delta migration: ALTER / new tables vs latest *_generated_from_entities.sql in this directory.

-- Table: categories
ALTER TABLE categories ADD COLUMN IF NOT EXISTS sort_order INTEGER NOT NULL DEFAULT 0;


