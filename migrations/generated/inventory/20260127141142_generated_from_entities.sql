-- Migration: Generated from Lifeguard entities
-- Service: inventory
-- Version: 20260127141142
-- Generated: 2026-01-27 14:11:42 UTC

-- This migration was automatically generated from entity definitions.
-- DO NOT EDIT MANUALLY - regenerate from entities instead.

-- Table: categories
CREATE TABLE IF NOT EXISTS categories (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(50) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    parent_id UUID REFERENCES categories(id) ON DELETE SET NULL,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_categories_code ON categories(code);
CREATE INDEX idx_categories_name ON categories(name);
CREATE INDEX idx_categories_is_active ON categories(is_active);
COMMENT ON TABLE categories IS 'Product categories for inventory organization';


-- Table: products
CREATE TABLE IF NOT EXISTS products (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sku VARCHAR(100) NOT NULL UNIQUE,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    category_id UUID REFERENCES categories(id) ON DELETE RESTRICT,
    price NUMERIC(19, 4) NOT NULL DEFAULT 0,
    currency_code VARCHAR(3) NOT NULL DEFAULT 'USD',
    stock_quantity INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_products_sku ON products(sku);
CREATE INDEX idx_products_category_id ON products(category_id);
CREATE INDEX idx_products_name ON products(name);
CREATE INDEX idx_products_is_active ON products(is_active);
COMMENT ON TABLE products IS 'Products in the inventory system';


-- Table: inventory_items
CREATE TABLE IF NOT EXISTS inventory_items (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    location_code VARCHAR(50) NOT NULL,
    batch_number VARCHAR(100) NOT NULL,
    quantity INTEGER NOT NULL DEFAULT 1,
    status VARCHAR(50) NOT NULL,
    expiry_date DATE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(product_id, location_code, batch_number)
);

CREATE INDEX idx_inventory_items_product_id ON inventory_items(product_id);
CREATE INDEX idx_inventory_items_location_code ON inventory_items(location_code);
CREATE INDEX idx_inventory_items_status ON inventory_items(status);
CREATE INDEX idx_inventory_items_expiry_date ON inventory_items(expiry_date);
COMMENT ON TABLE inventory_items IS 'Individual inventory items with location tracking';


