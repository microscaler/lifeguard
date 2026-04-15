-- Migration: Generated from Lifeguard entities
-- Service: inventory
-- Version: 20260415162603
-- Generated: 2026-04-15 16:26:03 UTC

-- This migration was automatically generated from entity definitions.
-- DO NOT EDIT MANUALLY - regenerate from entities instead.

-- Delta migration: ALTER / new tables vs merged *_generated_from_entities.sql history in this directory.

-- Table: categories
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_categories_code ON categories(code);
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_categories_name ON categories(name);
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_categories_is_active ON categories(is_active);


-- Table: products
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_products_sku ON products(sku);
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_products_category_id ON products(category_id);
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_products_name ON products(name);
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_products_is_active ON products(is_active);


-- Table: inventory_items
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_inventory_items_product_id ON inventory_items(product_id);
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_inventory_items_location_code ON inventory_items(location_code);
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_inventory_items_status ON inventory_items(status);
CREATE INDEX IF NOT EXISTS IF NOT EXISTS idx_inventory_items_expiry_date ON inventory_items(expiry_date);


