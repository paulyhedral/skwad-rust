## 1. Persona CRUD and lookup

- [ ] 1.1 Add `Settings::add_persona(&mut self, name: impl Into<String>, instructions: impl Into<String>) -> Result<&Persona>` appending a `user`/`enabled` persona and persisting; verify with a test that the returned/stored persona has the given name/instructions, type `user`, state `enabled`
- [ ] 1.2 Add `Settings::update_persona(&mut self, id: Uuid, name: impl Into<String>, instructions: impl Into<String>) -> Result<()>` rewriting name/instructions for an existing id of any type, no-op `Ok(())` if absent; verify with a test covering both an existing id (fields change) and an absent id (no error, no mutation)
- [ ] 1.3 Add `Settings::persona(&self, id: Uuid) -> Option<&Persona>` resolving only against `active_personas()`; verify with a test that a deleted persona's id returns `None` while an enabled/disabled one returns `Some`

## 2. Delete and restore

- [ ] 2.1 Add `Settings::remove_persona(&mut self, id: Uuid) -> Result<()>`: soft delete (`state = Deleted`, record kept) when `persona_type == System`, hard delete (record removed) when `persona_type == User`, no-op `Ok(())` if the id is absent; verify with tests for both branches plus the absent-id no-op
- [ ] 2.2 Add `Settings::restore_default_personas(&mut self) -> Result<()>` resetting every shipped default already present (by id, including soft-deleted) to its shipped name/instructions/type/state, appending any shipped default entirely missing, leaving non-default entries untouched; verify with a test that a renamed+disabled shipped persona reverts and a user persona is unaffected

## 3. Verify

- [ ] 3.1 Run `cargo +nightly fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`, confirm all pass
