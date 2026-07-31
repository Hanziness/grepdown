-- Migration: Add anchor column to links table
-- Purpose: Support anchor-aware link resolution for heading navigation
-- The anchor column stores the fragment identifier (e.g., "section-name" from "#section-name")
-- NULL means no anchor was specified in the link

ALTER TABLE links ADD COLUMN anchor TEXT;
