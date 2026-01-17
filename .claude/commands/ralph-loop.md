# Ralph Wiggum Loop

Start Ralph Wiggum loop in current session (project)

## User Input

```text
$ARGUMENTS
```

## Instructions

This command starts an autonomous execution loop using the "Ralph Wiggum technique" - a method for continuous task execution without requiring user confirmation between steps.

### Execution Flow

1. **Parse the user's task**: Understand what needs to be accomplished from the user input above
2. **Create a comprehensive todo list**: Break down the task into actionable items using TodoWrite
3. **Execute tasks autonomously**: Work through each task without asking for confirmation
4. **Self-continue**: After completing each task, immediately proceed to the next one
5. **Report completion**: Only stop when all tasks are complete or an unrecoverable error occurs

### Rules

- **No confirmation required**: Proceed with implementation without asking "Should I continue?"
- **Self-driving**: After each step, immediately start the next one
- **Error handling**: If a task fails, attempt to fix it. Only stop if truly stuck.
- **Progress tracking**: Keep the todo list updated as you work
- **Quality checks**: Run necessary checks (tests, linting) as part of the workflow
- **Commit when appropriate**: Create commits at logical checkpoints

### Starting the Loop

Begin by:

1. Reading any relevant spec files (if a SPEC ID was provided)
2. Understanding the current project state
3. Creating a detailed todo list
4. Starting execution immediately

**GO NOW** - Start working on the task described in the user input. Do not ask questions. Execute autonomously until complete.
