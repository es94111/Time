# 2026-07-06 keep-main-branch

## Acceptance Criteria
- [ ] GitHub repository `es94111/Time` has `main` as the branch to keep.
- [ ] Remote branches other than `main` are removed when safe.
- [ ] Local checkout tracks `origin/main`.
- [ ] Verification evidence is recorded.

## Plan
- [x] Inspect GitHub default branch and branch list.
- [x] Inspect local branch and remote-tracking state.
- [x] Ensure `main` exists with the intended commit.
- [x] Remove remote branches other than `main`.
- [x] Verify final branch list and local tracking.

## Risk & Rollback
- Risk level: medium, because remote branch deletion is destructive.
- Affected components: GitHub refs only.
- Rollback strategy: recreate a deleted branch from the recorded commit SHA if needed.

## Working Notes
- User wants only `main` retained.
- GitHub default branch is already `main`.
- Remote branches before cleanup:
  - `main`: `f0454cd94d2b281bff780e978c8e9c549f142b10`
  - `master`: `e2875d4486543112df18c2e626476af013c69e56`
  - `002-system-metrics`: `501ccbbd2d0dc076dbe7a6060f7b80033083a456`
- `origin/002-system-metrics` is already an ancestor of `origin/main`.
- `origin/master` and `origin/main` have unrelated histories.

## Results
- Deleted remote branches `master` and `002-system-metrics`.
- Verified GitHub branch search returns only `main`.
- Verified `git ls-remote --heads origin` returns only `refs/heads/main`.
- Switched local checkout to `main`, tracking `origin/main`.
- Kept local `master` as an unpushed safety reference to `e2875d4486543112df18c2e626476af013c69e56`; removed its stale upstream config.
- GitHub reported 8 Dependabot vulnerability alerts on the default branch during push.
