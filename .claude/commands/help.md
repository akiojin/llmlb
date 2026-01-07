# Ralph Wiggum Help

Explain Ralph Wiggum technique and available commands (project)

## The Ralph Wiggum Technique

The "Ralph Wiggum technique" is an autonomous execution pattern for Claude Code that enables continuous task execution without requiring user confirmation between steps.

### Philosophy

Named after the Simpsons character known for his unexpected actions, this technique allows Claude to work autonomously by:

1. **Self-driving execution**: Once started, Claude continues working without asking "Should I proceed?"
2. **Automatic continuation**: After completing each task, immediately start the next one
3. **Intelligent error handling**: Attempt to fix issues rather than stopping to ask for help
4. **Progress transparency**: Keep todo lists and status updated for visibility

### Available Commands

| Command | Description |
|---------|-------------|
| `/ralph-loop <task>` | Start autonomous execution loop with the given task |
| `/cancel-ralph` | Stop the autonomous loop and return to interactive mode |
| `/help` | Display this help information |

### Usage Examples

```text
/ralph-loop SPEC-12345678の実装を進めて
/ralph-loop Fix all failing tests in the project
/ralph-loop Implement the user authentication feature
```

### When to Use

- Large implementation tasks with many steps
- Spec-driven development with clear requirements
- Tasks where the path forward is well-defined
- When you want hands-off execution

### When NOT to Use

- Exploratory or research tasks requiring human judgment
- Tasks with ambiguous requirements
- Situations requiring frequent user input or decisions
- When you need to review each step carefully

### Safety

The loop can be cancelled at any time using `/cancel-ralph` or by interrupting Claude's response.
