# Phase 1 backlog

Each row becomes one issue and one pull request, in this order. Each row gets a full story like [US-001](US-001.md) before its code starts. A row starts when the owner merges the rows it needs.

Roles: **E** employee, **M** manager, **A** admin. A higher role can do everything a lower role can.

## Foundation

| ID | Task | Needs | Done when |
| --- | --- | --- | --- |
| T-01 | This plan | | The owner agrees with it |
| T-02 | Rust workspace, CI, money type | T-01 | CI fails on a failing test, a lint or bad formatting |
| T-03 | Bill calculation | T-02 | Discount, service charge and VAT (none, inside, on top) match hand-worked examples |
| T-04 | Roles and permissions | T-02 | Each permission is tested for each role |
| T-05 | Order item state machine | T-04 | Every allowed and refused move is tested |
| T-06 | PromptPay QR text | T-02 | Output equals an independent generator for mobile, tax ID and e-wallet |
| T-07 | Trace ID and error format | T-02 | One ID appears in the log, the response header and the error body |
| T-08 | Database schema, ORM and migration | T-03, T-04, T-05 | The same tests pass on Postgres and SQLite |
| T-09 | Server, health check, Dockerfile, Compose | T-07, T-08 | CI starts Compose and the health check answers |
| T-10 | React app shell, Thai and English | T-09 | A missing translation fails the type check |

## Stories

| ID | Story | Role | Needs | Done when |
| --- | --- | --- | --- | --- |
| [US-001](US-001.md) | Sign in and sign out | E | T-10 | Five wrong passwords lock the account for 15 minutes. Passwords use Argon2id |
| US-002 | Manage users and roles | A | US-001 | A removed user loses access on the next request |
| US-003 | Switch between Thai and English | E | T-10 | The choice survives a reload |
| US-004 | Shop settings, VAT, service charge, PromptPay ID | A | US-001 | A PromptPay ID with a wrong check digit is refused |
| US-005 | Manage the menu and mark an item sold out | M, E | US-001 | A sold-out item cannot be ordered |
| US-006 | Manage tables | M | US-001 | Each table has its own secret customer link |
| US-007 | Select a table and open an order | E | US-006 | Two people opening one table at once get one order |
| US-008 | Add items with a note | E | US-005, US-007 | Sending twice adds the dish once |
| US-009 | Takeaway order | E | US-008 | It needs no table |
| US-010 | Kitchen screen on a phone | E | US-008 | A new dish appears within 2 seconds without a reload |
| US-011 | Void an item with a remark | E, M | US-008 | A started dish needs a manager |
| US-012 | Move a table and set it back, with a remark | E | US-007 | The history shows both changes, who made them and why |
| US-013 | Bill and discount | M | US-008 | A discount needs a remark |
| US-014 | Cash payment with change | E | US-013 | Paying twice with one request records one payment |
| US-015 | PromptPay payment | E | US-013, T-06 | The QR asks for the exact amount left. Staff confirm after they see the money arrive |
| US-016 | Print a receipt | E | US-014 | It prints in the chosen language on 80 mm paper |
| US-017 | Refund with a remark | M | US-014 | The report no longer counts the sale |
| US-018 | Customer reads the menu by QR | Customer | US-006 | It works on a phone without signing in |
| US-019 | Customer sends an order by QR | Customer | US-018 | Nothing reaches the kitchen or the bill until staff accept it |
| US-020 | Daily sales report | M | US-014 | Totals equal the sum of the receipts |
| US-021 | Audit log | A | US-002 | Searching a trace ID finds every row of that request |
| US-022 | Install on a phone as an app | E | US-010 | It opens from the home screen |

## Later

Ordered by how many shops I expect to need them. This order is a guess. The owner may reorder it.

1. Cash drawer shift: open with a float, close with a count.
2. Menu options such as spice level and extra egg.
3. Receipt printer over the network (ESC/POS).
4. Split a bill by person.
5. Stock by ingredient.
6. Automatic PromptPay confirmation. It needs a contract and API keys from a bank.
7. e-Tax invoice for the Revenue Department.
8. Members and points.
9. Order while the server is unreachable.
10. Several branches.
