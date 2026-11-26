//! The state of one item on an order, and the moves between states.
//!
//! Types in this file:
//!
//! | Type          | Holds                                                     | Why                                                    |
//! | ------------- | --------------------------------------------------------- | ------------------------------------------------------ |
//! | `ItemStatus`  | Where one item is, from a customer's request to the table | `ItemStatus::moves` lists every allowed move in one place. |
//! | `RefusedMove` | The two states of a move that is not allowed              | `check_move` returns it inside a `Result`. The compiler warns when a caller ignores a `Result`. |
//!
//! States:
//!
//! | State       | Means                                                            | On the bill |
//! | ----------- | ---------------------------------------------------------------- | ----------- |
//! | `Requested` | A customer sent it from the table's QR link. It waits for staff. | No          |
//! | `Queued`    | Staff accepted or entered it. The kitchen has not started it.    | Yes         |
//! | `Cooking`   | The kitchen started it.                                          | Yes         |
//! | `Ready`     | It is done and waits for a waiter.                               | Yes         |
//! | `Served`    | It is at the table.                                              | Yes         |
//! | `Voided`    | It was removed. This state is final.                             | No          |
//!
//! Moves:
//!
//! - Forward one step needs `Operate`: requested to queued, queued to cooking, cooking to ready,
//!   ready to served. Queued may also go straight to ready, for an item that needs no cooking, such as a drink.
//! - Back one step after a mis-tap needs `Operate`: ready to cooking, served to ready.
//! - Back from cooking to queued needs `VoidStartedItem`. With `Operate`, an employee could set a
//!   started item back to queued and then void it without a manager.
//! - Void from requested or queued needs `Operate`. Voiding a requested item is how staff refuse a customer's request.
//! - Void from cooking, ready or served needs `VoidStartedItem`.
//! - Every other move is refused: a move to the same state, any move out of voided, and any move back to requested.

use crate::role::Permission;

/// Where one order item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemStatus {
    Requested,
    Queued,
    Cooking,
    Ready,
    Served,
    Voided,
}

/// A move that the rules do not allow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefusedMove {
    pub from: ItemStatus,
    pub to: ItemStatus,
}

impl ItemStatus {
    /// The moves out of this state, each with the permission it needs.
    fn moves(self) -> &'static [(ItemStatus, Permission)] {
        use ItemStatus::{Cooking, Queued, Ready, Requested, Served, Voided};
        use Permission::{Operate, VoidStartedItem};

        match self {
            Requested => &[(Queued, Operate), (Voided, Operate)],
            Queued => &[(Cooking, Operate), (Ready, Operate), (Voided, Operate)],
            Cooking => &[(Ready, Operate), (Queued, VoidStartedItem), (Voided, VoidStartedItem)],
            Ready => &[(Served, Operate), (Cooking, Operate), (Voided, VoidStartedItem)],
            Served => &[(Ready, Operate), (Voided, VoidStartedItem)],
            Voided => &[],
        }
    }

    /// The permission a person needs to move an item from this state to `to`.
    /// Returns `RefusedMove` when no role may make the move.
    pub fn check_move(self, to: ItemStatus) -> Result<Permission, RefusedMove> {
        for &(next, permission) in self.moves() {
            if next == to {
                return Ok(permission);
            }
        }
        Err(RefusedMove { from: self, to })
    }

    /// Whether the item counts on the bill.
    pub fn is_billed(self) -> bool {
        match self {
            ItemStatus::Requested => false,
            ItemStatus::Queued => true,
            ItemStatus::Cooking => true,
            ItemStatus::Ready => true,
            ItemStatus::Served => true,
            ItemStatus::Voided => false,
        }
    }
}

/// OK cases check an allowed move or a right answer. NG cases check that a move is refused.
#[cfg(test)]
mod tests {
    use super::{ItemStatus, RefusedMove};
    use crate::role::{Permission, Role};
    use ItemStatus::{Cooking, Queued, Ready, Requested, Served, Voided};

    /// The rows and the columns of `TABLE`, in this order.
    const STATES: [ItemStatus; 6] = [Requested, Queued, Cooking, Ready, Served, Voided];

    const NO: Option<Permission> = None;
    const OPERATE: Option<Permission> = Some(Permission::Operate);
    const VOID_STARTED: Option<Permission> = Some(Permission::VoidStartedItem);

    /// The whole state machine. The row is the state an item is in. The column is the state
    /// it moves to. Each cell is the permission the move needs, or `NO` when it is refused.
    #[rustfmt::skip]
    const TABLE: [[Option<Permission>; 6]; 6] = [
        //  to:          requested  queued        cooking   ready     served    voided
        /* requested */ [NO,        OPERATE,      NO,       NO,       NO,       OPERATE],
        /* queued    */ [NO,        NO,           OPERATE,  OPERATE,  NO,       OPERATE],
        /* cooking   */ [NO,        VOID_STARTED, NO,       OPERATE,  NO,       VOID_STARTED],
        /* ready     */ [NO,        NO,           OPERATE,  NO,       OPERATE,  VOID_STARTED],
        /* served    */ [NO,        NO,           NO,       OPERATE,  NO,       VOID_STARTED],
        /* voided    */ [NO,        NO,           NO,       NO,       NO,       NO],
    ];

    /// The table as one (from, to, expected) case per cell. 6 times 6 is 36 cases.
    fn cases() -> Vec<(ItemStatus, ItemStatus, Option<Permission>)> {
        let mut cases = Vec::new();
        for row in 0..6 {
            for column in 0..6 {
                cases.push((STATES[row], STATES[column], TABLE[row][column]));
            }
        }
        cases
    }

    /// Every state a person with `role` can reach from `start`, one allowed move at a time.
    fn reachable(role: Role, start: ItemStatus) -> Vec<ItemStatus> {
        let mut reached = vec![start];
        let mut next_to_visit = 0;
        while next_to_visit < reached.len() {
            let from = reached[next_to_visit];
            for to in STATES {
                let mut allowed = false;
                if let Ok(permission) = from.check_move(to) {
                    allowed = role.can(permission);
                }
                if allowed && !reached.contains(&to) {
                    reached.push(to);
                }
            }
            next_to_visit += 1;
        }
        reached
    }

    // ---------- moves ----------

    /// OK: each cell with a permission is allowed and needs exactly that permission. That is 13 cells.
    #[test]
    fn ok_allowed_moves_need_the_permission_in_the_table() {
        let mut checked = 0;
        for (from, to, expected) in cases() {
            if let Some(permission) = expected {
                assert_eq!(from.check_move(to), Ok(permission), "{from:?} to {to:?}");
                checked += 1;
            }
        }
        assert_eq!(checked, 13);
    }

    /// NG: each cell marked `NO` is refused, and the refusal names both states. That is 23 cells.
    #[test]
    fn ng_moves_marked_no_are_refused() {
        let mut checked = 0;
        for (from, to, expected) in cases() {
            if expected.is_none() {
                assert_eq!(from.check_move(to), Err(RefusedMove { from, to }), "{from:?} to {to:?}");
                checked += 1;
            }
        }
        assert_eq!(checked, 23);
    }

    /// OK: an employee can void an item before cooking starts. A manager can void it at any state.
    #[test]
    fn ok_void_before_cooking_needs_no_manager() {
        for start in [Requested, Queued] {
            assert!(reachable(Role::Employee, start).contains(&Voided), "employee from {start:?}");
        }
        for start in [Requested, Queued, Cooking, Ready, Served] {
            assert!(reachable(Role::Manager, start).contains(&Voided), "manager from {start:?}");
        }
    }

    /// NG: an employee cannot void a started item, not even through a chain of moves
    /// such as served to ready to cooking to queued to voided.
    #[test]
    fn ng_employee_cannot_void_a_started_item_by_any_path() {
        for start in [Cooking, Ready, Served] {
            let reached = reachable(Role::Employee, start);
            assert!(!reached.contains(&Voided), "from {start:?} an employee reaches {reached:?}");
        }
    }

    // ---------- bill ----------

    /// OK: a requested item waits for staff and a voided item is removed, so neither is on the bill.
    #[test]
    fn ok_bill_counts_only_queued_cooking_ready_and_served() {
        let cases =
            [(Requested, false), (Queued, true), (Cooking, true), (Ready, true), (Served, true), (Voided, false)];
        for (status, billed) in cases {
            assert_eq!(status.is_billed(), billed, "{status:?}");
        }
    }
}
