# User stories and how we track them

## The rule

One story or task is one issue. One issue has one pull request. The owner reviews each pull request before the next one starts.

## Who holds what

| Place | Holds | Read by |
| --- | --- | --- |
| Issue | The story, its acceptance checks, its status and discussion | People, and AI through the API |
| Pull request | The code change. Its body says `Closes #N` | Reviewer |
| `docs/stories/US-NNN.md` | What was built, what changed later, and why | AI and people, offline, from a clone |
| `docs/decisions/` | A technical choice that is costly to undo | Both |

Only an issue says whether work is open or done. Only the story file says what the product does today. Neither repeats the other.

## Steps for one story

1. Open an issue from the "User story" form. The title is `US-012: Move a table and set it back`.
2. Edit the issue body while the story is still being discussed. Add a comment that says what changed and why.
3. Create a branch `feat/<issue>-<short-name>`.
4. Open the pull request with `Closes #<issue>`. It adds or updates the story file in the same pull request.
5. The owner reviews. The merge closes the issue.
6. If the story changes later, open a new issue that links the old one. Do not reopen the old one. Add a row to the story file's history.

## How much detail an issue needs

[US-001](US-001.md) is the example. An issue is ready when a person who has never seen the product could build and test it without asking. It names each box and button, the Thai and English text, each state, what stops a double API call, the API and its errors, the security rules and the checks. The issue form has a required field for each.

A story too large for one review is split into parts such as US-001a. Each part is one issue and one pull request.

## Why issues are closed and not edited forever

A closed issue says what we believed at that time. If someone rewrites it later, the reason for the first version is lost. A new issue and a history row keep both versions. `git log -p docs/stories/US-012.md` then shows each change, its reason and its pull request, so an AI can read the past without the network.

## Story file

Copy this when a story's pull request starts.

```markdown
# US-012: Move a table and set it back

As an employee, I want to move an order to another table and set it back, so that a mistake costs one tap.

## Behaviour today

- ...

## Checks

| Check | Proved by |
| --- | --- |
| A move needs a remark | `backend/.../tests/move.rs::move_needs_remark` |

## History

| Date | What changed | Why | Issue | PR |
| --- | --- | --- | --- | --- |
| 2026-11-25 | First version | Waiters seat guests at the wrong table | #0 | #0 |
```
