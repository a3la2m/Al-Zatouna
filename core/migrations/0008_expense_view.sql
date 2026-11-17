-- Phase 21: the expenses list required `expenses.view`, which was never
-- seeded — so even system_admin (who receives every seeded permission) could
-- not list expenses. Seed it and grant it to the finance roles.
INSERT OR IGNORE INTO permissions (code, name, grp) VALUES ('expenses.view','عرض المصروفات','money');
INSERT OR IGNORE INTO role_permissions (role_id, permission_id)
    SELECT r.id, p.id FROM roles r, permissions p
    WHERE r.code IN ('system_admin','center_manager','accountant') AND p.code='expenses.view';
