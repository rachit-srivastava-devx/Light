# Initializer brief — run once as the FIRST session of <PROJECT>

You are the initializer agent. You write no feature code. You build the environment every later session depends on.

1. Read the blueprint: `<BLUEPRINT_PATH>/README.md` and `<BLUEPRINT_PATH>/episodes/Episode-01*.md` (skim later episodes for scope).
2. Write `feature_list.json`: a comprehensive, granular list of end-to-end features for the CURRENT episode (aim small: 20-60 entries per episode; each `steps` array is executable by a user). All `"passes": false`. Category values: functional | ui | api | ops.
3. Scaffold the project per the blueprint's Episode-01 architecture decisions (language, framework, layout). Minimal — it must run, not impress.
4. Write `./init.sh`: installs deps if missing, starts the dev server/CLI in a known state, prints the one smoke command.
5. Write `claude-progress.txt` (first entry: what you set up, how to run it, what's next).
6. Set the gate: create a `verify` script (format + lint + typecheck + unit + contract tests, <10 min — Company-OS C17/T19) and put it in `.fleet/gate.cmd` (e.g. `npm run -s verify`). Module slices follow FOLDER-STRUCTURE.md §1 (contracts/ core/ adapters/memory/ …); every data-touching module includes the tenant-bleed test (T13) in the gate.
7. `git init` if needed; first commit: "initialize <PROJECT> environment".

Definition of done for THIS session: a fresh agent with zero context can run `./init.sh`, read two files, and start feature #1 within 2 minutes.
Output `<promise>INIT-COMPLETE</promise>` when done.
