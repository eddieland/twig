# Agent Skills

This directory contains [Agent Skills](https://agentskills.io/) — portable, version-controlled
packages of procedural knowledge that AI agents can discover and load on demand.

## Structure

Each subdirectory is a self-contained skill with a required `SKILL.md` file:

```
.skills/
├── README.md              # This file
├── <skill-name>/
│   ├── SKILL.md           # Required: metadata + instructions
│   ├── scripts/           # Optional: executable code
│   ├── references/        # Optional: additional documentation
│   └── assets/            # Optional: templates, data files, schemas
```

## How it works

1. **Discovery** — Agents scan this directory and read each skill's `name` and `description` from
   the YAML frontmatter in `SKILL.md` (~100 tokens per skill).
2. **Activation** — When a task matches a skill's description, the agent loads the full `SKILL.md`
   body into context.
3. **Execution** — The agent follows the instructions, optionally loading referenced files from
   `scripts/`, `references/`, or `assets/` as needed.

## Adding a new skill

1. Create a new directory whose name matches the skill's `name` field (lowercase, hyphens only).
2. Add a `SKILL.md` with required YAML frontmatter:

```yaml
---
name: my-skill
description: What this skill does and when to use it.
---

# My Skill

Instructions for the agent...
```

3. Optionally add `scripts/`, `references/`, and `assets/` subdirectories.

## Specification

See the full [Agent Skills specification](https://agentskills.io/specification) for naming
conventions, frontmatter fields, and best practices.
