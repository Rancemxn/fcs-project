# Agent Skills for Context Engineering Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Vendor the existing Agent Skills for Context Engineering collection, install its 17 project-local skills, and document precise activation timing in the repository root `AGENTS.md`.

**Architecture:** Keep one complete source copy under `.agents/vendor/Agent-Skills-for-Context-Engineering/`; copy only the immediate skill directories into `.agents/skills/` so the host can discover each `SKILL.md` directly. Add routing guidance to `AGENTS.md` without changing existing instructions or FCS source behavior.

**Tech Stack:** Markdown, JSON manifests, PowerShell filesystem operations, and the upstream Python validation scripts.

---

### Task 1: Normalize the vendored source location

**Files:**
- Move: `Agent-Skills-for-Context-Engineering-main/` → `.agents/vendor/Agent-Skills-for-Context-Engineering/`

- [x] **Step 1: Move the existing complete source directory**

  Run:

  ```powershell
  New-Item -ItemType Directory -Force .agents/vendor | Out-Null
  Move-Item -LiteralPath Agent-Skills-for-Context-Engineering-main -Destination .agents/vendor/Agent-Skills-for-Context-Engineering
  ```

- [x] **Step 2: Verify the source boundary**

  Run:

  ```powershell
  Test-Path .agents/vendor/Agent-Skills-for-Context-Engineering
  Test-Path Agent-Skills-for-Context-Engineering-main
  Test-Path .agents/vendor/Agent-Skills-for-Context-Engineering/.plugin/plugin.json
  ```

  Expected: `True`, `False`, `True`.

### Task 2: Install the project-local skills

**Files:**
- Create: `.agents/skills/<skill-name>/` for each directory under the vendored `skills/` directory

- [x] **Step 1: Copy every upstream skill directory without flattening**

  Run:

  ```powershell
  $source = (Resolve-Path .agents/vendor/Agent-Skills-for-Context-Engineering/skills).Path
  $destination = (Resolve-Path .agents/skills).Path
  Get-ChildItem -LiteralPath $source -Directory | ForEach-Object {
      Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $destination $_.Name) -Recurse -Force
  }
  ```

- [x] **Step 2: Verify the installed skill count and required files**

  Run:

  ```powershell
  $installed = Get-ChildItem .agents/skills -Directory | Where-Object { Test-Path (Join-Path $_.FullName SKILL.md) }
  $installed.Count
  $installed | ForEach-Object { $_.Name }
  ```

  Expected: count `17`; every listed directory has a direct `SKILL.md`.

### Task 3: Add activation guidance to the root instructions

**Files:**
- Modify: `AGENTS.md`

- [x] **Step 1: Append the Context Engineering routing section**

  Add a section that names `.agents/skills/` as the project-local discovery directory, requires progressive disclosure and minimal relevant activation, preserves FCS specifications and Trellis instructions as higher-priority project rules, and maps all 17 skills to positive triggers plus adjacent non-triggers.

- [x] **Step 2: Check that existing instructions are preserved**

  Run:

  ```powershell
  git diff -- AGENTS.md
  ```

  Expected: the existing uncommitted content remains, followed by only the new routing section.

### Task 4: Validate the integration

**Files:**
- Read: `.agents/vendor/Agent-Skills-for-Context-Engineering/researcher/scripts/validate_platform_compat.py`
- Read: `.agents/vendor/Agent-Skills-for-Context-Engineering/researcher/scripts/validate_repo.py`

- [x] **Step 1: Validate frontmatter and platform layout**

  Run from the vendored project root:

  ```powershell
  python researcher/scripts/validate_platform_compat.py --require-reference-validator
  ```

  The strict form was attempted but the environment does not have the optional `skills-ref` CLI. The same validator without the external-reference requirement passed for all 17 skills and 4 local layouts.

- [x] **Step 2: Validate the repository's deterministic structure**

  Run:

  ```powershell
  python researcher/scripts/validate_repo.py --strict
  ```

- [x] **Step 3: Compare installed skill directories with their sources**

  Run:

  ```powershell
  $source = (Resolve-Path .agents/vendor/Agent-Skills-for-Context-Engineering/skills).Path
  $destination = (Resolve-Path .agents/skills).Path
  $mismatches = foreach ($skill in Get-ChildItem $source -Directory) {
      $srcHash = (Get-ChildItem $skill.FullName -Recurse -File | Get-FileHash | ForEach-Object Hash) -join ''
      $dstHash = (Get-ChildItem (Join-Path $destination $skill.Name) -Recurse -File | Get-FileHash | ForEach-Object Hash) -join ''
      if ($srcHash -ne $dstHash) { $skill.Name }
  }
  $mismatches
  ```

  Expected: no output.
