-- Rollback of the initial schema: drop in reverse dependency order.

DROP TABLE snippet_fts;
DROP TABLE ai_action;
DROP TABLE sync_record;
DROP TABLE device;
DROP TABLE app_rule;
DROP TABLE template_field;
DROP TABLE snippet_tag;
DROP TABLE snippet;
DROP TABLE tag;
DROP TABLE folder;
