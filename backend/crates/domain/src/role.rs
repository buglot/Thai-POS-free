//! Roles and permissions.
//!
//! Each person has one role. A handler asks for a `Permission`, never for a `Role`.
//! The rule for who may do what lives in one function, `Permission::lowest_role`.
//!
//! Types in this file:
//!
//! | Type         | Holds                                   | Why                                                          |
//! | ------------ | --------------------------------------- | ------------------------------------------------------------ |
//! | `Role`       | `Employee`, `Manager` or `Admin`        | The variants are written from least to most power. The derived `Ord` compares them in that order, so a higher role holds every permission of a lower role. |
//! | `Permission` | One action that needs more than sign-in | Handlers name it. Changing who may do an action changes one line here, not every handler. |

/// A person's job in the shop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    Employee,
    Manager,
    Admin,
}

/// One action that needs more than being signed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Daily work. Open an order, add and send items, accept or refuse a customer's request,
    /// mark a dish cooking, ready or served, void an item before cooking starts, move a table,
    /// mark a menu item sold out, take cash and PromptPay (พร้อมเพย์) payments, print a receipt.
    Operate,
    /// Void an item after the kitchen started it. Used by US-011.
    VoidStartedItem,
    /// Give a discount (ส่วนลด) on a bill. Used by US-013.
    ApplyDiscount,
    /// Refund a paid bill. Used by US-017.
    Refund,
    /// Add, change and remove menu items and their prices. Used by US-005.
    ManageMenu,
    /// Add and remove tables and their customer links. Used by US-006.
    ManageTables,
    /// See the daily sales report. Used by US-020.
    ViewReports,
    /// Create users, change their role and remove them. Used by US-002.
    ManageUsers,
    /// Change VAT (ภาษีมูลค่าเพิ่ม), service charge (ค่าบริการ) and the PromptPay ID. Used by US-004.
    ManageSettings,
    /// Read the audit log. Used by US-021.
    ViewAudit,
}

impl Permission {
    /// The lowest role that holds this permission.
    fn lowest_role(self) -> Role {
        match self {
            Permission::Operate => Role::Employee,
            Permission::VoidStartedItem => Role::Manager,
            Permission::ApplyDiscount => Role::Manager,
            Permission::Refund => Role::Manager,
            Permission::ManageMenu => Role::Manager,
            Permission::ManageTables => Role::Manager,
            Permission::ViewReports => Role::Manager,
            Permission::ManageUsers => Role::Admin,
            Permission::ManageSettings => Role::Admin,
            Permission::ViewAudit => Role::Admin,
        }
    }
}

impl Role {
    /// Whether a person with this role may do `permission`.
    pub fn can(self, permission: Permission) -> bool {
        self >= permission.lowest_role()
    }
}

/// OK cases check that a role holds a permission. NG cases check that a role is refused one.
#[cfg(test)]
mod tests {
    use super::{Permission, Role};

    /// The whole matrix. Each row is a permission, then whether an employee, a manager
    /// and an admin hold it. Add a row when you add a permission.
    #[rustfmt::skip]
    const TABLE: [(Permission, bool, bool, bool); 10] = [
        // permission                  employee  manager  admin
        (Permission::Operate,          true,     true,    true),
        (Permission::VoidStartedItem,  false,    true,    true),
        (Permission::ApplyDiscount,    false,    true,    true),
        (Permission::Refund,           false,    true,    true),
        (Permission::ManageMenu,       false,    true,    true),
        (Permission::ManageTables,     false,    true,    true),
        (Permission::ViewReports,      false,    true,    true),
        (Permission::ManageUsers,      false,    false,   true),
        (Permission::ManageSettings,   false,    false,   true),
        (Permission::ViewAudit,        false,    false,   true),
    ];

    /// The table as one (role, permission, holds) case per cell.
    fn cases() -> Vec<(Role, Permission, bool)> {
        let mut cases = Vec::new();
        for (permission, employee, manager, admin) in TABLE {
            cases.push((Role::Employee, permission, employee));
            cases.push((Role::Manager, permission, manager));
            cases.push((Role::Admin, permission, admin));
        }
        cases
    }

    /// OK: every cell marked `true` is allowed. That is 18 of the 30 cells.
    #[test]
    fn ok_role_holds_each_permission_marked_true() {
        let mut checked = 0;
        for (role, permission, holds) in cases() {
            if holds {
                assert!(role.can(permission), "{role:?} must hold {permission:?}");
                checked += 1;
            }
        }
        assert_eq!(checked, 18);
    }

    /// NG: every cell marked `false` is refused. That is 12 of the 30 cells.
    #[test]
    fn ng_role_is_refused_each_permission_marked_false() {
        let mut checked = 0;
        for (role, permission, holds) in cases() {
            if !holds {
                assert!(!role.can(permission), "{role:?} must not hold {permission:?}");
                checked += 1;
            }
        }
        assert_eq!(checked, 12);
    }
}
