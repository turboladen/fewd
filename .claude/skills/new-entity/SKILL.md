---
name: new-entity
description: Scaffold a new full-stack entity (migration, entity, service, DTO, routes, types, hooks, component)
disable-model-invocation: true
---

Scaffold a new entity: $ARGUMENTS

Follow the project's established patterns. For each step, use existing files as templates:

## Steps
1. Create migration in `server/migration/src/` (use `m20260214_000011_create_settings.rs` as template for naming)
2. Register migration in `server/migration/src/lib.rs`
3. Create entity in `server/src/entities/` (follow `person.rs` pattern)
4. Create service in `server/src/services/` (follow `person_service.rs` pattern)
5. Add DTOs in `server/src/dto.rs`
6. Create route handler in `server/src/routes/` (follow `people.rs` pattern)
7. Register routes in `server/src/routes/mod.rs`
8. Create TypeScript types in `src/types/`
9. Create hook in `src/hooks/` (follow `usePeople.ts` pattern)
10. Create UI component stub in `src/components/`

## Context
- DB status: !`ls server/migration/src/m*.rs | tail -3`
- Existing entities: !`ls server/src/entities/*.rs`
- Existing services: !`ls server/src/services/*_service.rs`

