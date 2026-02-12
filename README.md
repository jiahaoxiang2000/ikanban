# iKanban

Terminal-first AI task orchestration for software projects, built with Bun, Ink, and React.

## What It Does

iKanban helps you run coding tasks through an OpenCode runtime while keeping each task isolated in its own Git worktree. It tracks project/task state locally, supports iterative follow-up prompts, and provides a review + merge workflow from the terminal UI.

## Features

- Project selector for managing multiple Git repositories
- Kanban-style task lifecycle (`queued` -> `running` -> `review` -> `completed`/`failed`)
- Per-task Git worktree isolation
- Follow-up prompts on tasks in review
- Merge reviewed tasks back to the default branch
- Built-in runtime log panel
- Vim-style navigation keys across views

## Requirements

- [Bun](https://bun.sh/) (runtime + package manager)
- A Git repository for each project you manage

## Getting Started

```bash
bun install
bun run dev
```

The app starts in your terminal and uses your current directory as the default project when possible.

## Scripts

- `bun run dev` - run the CLI app
- `bun run build` - build to `dist/`
- `bun run typecheck` - run TypeScript type checks

## Keyboard Shortcuts

Global:

- `Ctrl+C` / `q` - quit
- `Tab` - switch between Project Selector and Task Board
- `l` - toggle log panel

Project Selector:

- `j`/`k` or arrow keys - move selection
- `Enter` - open selected project
- `n` - add a new project

Task Board:

- `j`/`k` or arrow keys - move selection
- `n` - create task
- `o` - pick model
- `p` - send follow-up prompt
- `m` - merge task
- `d` - delete task

Log Panel:

- `j`/`k` - line scroll
- `u`/`d` - page scroll
- `g`/`G` - oldest/newest log
- `v` - toggle info/debug visibility

## State Storage

iKanban stores local state under:

- `~/.ikanban/projects.json`
- `~/.ikanban/tasks.json`

## License

MIT
