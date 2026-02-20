# Software Specifications

This directory contains software specifications and technical documentation, largely created by Large Language Models
(LLMs) to guide development and maintain project context.

## Purpose

These specifications serve as:

- **Design blueprints** for features and components
- **Context repositories** for AI-assisted development
- **Knowledge checkpoints** for incremental progress tracking
- **Communication artifacts** between human developers and AI agents

## Document Naming Scheme

To maintain organization and enable chronological sorting, follow this naming convention for specification documents:

### Format

```plaintext
YYYY-MM-DD_descriptive-name.md
```

- **Date prefix**: ISO 8601 format (`YYYY-MM-DD`) ensures alphabetical sorting equals chronological sorting
- **Underscore separator**: Separates date from description
- **Descriptive name**: Lowercase, hyphen-separated, meaningful description
- **Extension**: `.md` for Markdown documents

### Examples

- `2025-10-30_github-data-client-plan.md` - Initial plan for GitHub data client
- `2025-10-30_authentication-spec.md` - Authentication implementation specification
- `2025-11-01_api-design-checkpoint.md` - Checkpoint after API design phase
- `2025-11-02_testing-strategy.md` - Testing approach and strategy
- `2025-11-05_lessons-learned-sprint-1.md` - Retrospective document

### Benefits

- **Sortable**: Files automatically sort chronologically in file explorers
- **Traceable**: Easy to see when a specification was created
- **Discoverable**: Descriptive names make content obvious at a glance
- **Version-friendly**: Date prefix prevents name collisions for evolving specs

## Specification Template

To bootstrap new documents quickly, copy [`_TEMPLATE.md`](_TEMPLATE.md) into a dated filename that matches the naming
scheme above, then replace the placeholders with project-specific details. The template mirrors the structure of our
existing plans, including purpose, constraints, backlog, risk tracking, and a space for lessons learned.

## Intentional Context Compaction

When working with LLM agents on complex tasks, we recommend a practice called **intentional context compaction**. This
approach significantly improves outcomes by:

### Key Principles

1. **Incremental Steps**: Break down large tasks into smaller, manageable steps

   - Request the agent to complete one logical unit of work at a time
   - Verify each step before proceeding to the next
   - Prevent context drift and accumulating errors

1. **Regular Checkpointing**: Have the agent periodically save its progress

   - Document what has been accomplished
   - Record key decisions and rationale
   - Note any challenges or blockers encountered

1. **Lessons Learned**: Capture insights for future agents

   - Summarize what worked well
   - Identify patterns that should be followed
   - Document pitfalls to avoid
   - Record useful techniques or approaches

### Benefits

- **Continuity**: New agents can quickly understand project state
- **Efficiency**: Reduces redundant work and context rebuilding
- **Quality**: Enables review and course-correction at each step
- **Knowledge Transfer**: Preserves institutional knowledge across sessions

### Example Workflow

```plaintext
1. Agent completes initial research → checkpoint created
2. Agent designs architecture → checkpoint updated with design decisions
3. Agent implements component A → checkpoint with implementation notes
4. Agent tests component A → checkpoint with test results and lessons
5. Next agent reviews checkpoints → continues with component B
```

By practicing intentional context compaction, you create a self-documenting development process that scales across
multiple sessions and agent interactions.
