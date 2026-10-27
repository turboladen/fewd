# Agent teams and subagents

These hold when a lead session drives teammates or subagents through worktrees.

- **Act on the tree, not the message.** Reports, hand-backs and "finished" notices can arrive several minutes before the writes they describe. A unit is done when its worktree's HEAD and clean status match the report. Wait for that with a background until-loop, and recheck a file that is missing before calling a claim false.
- **Keep shared state in files.** Each unit gets a run directory under the session scratchpad with `decisions.md` (the lead's rulings, append-only), `status.md` (the author's phase, SHA and gate state) and one file per review round. A ruling sent only by message can land after the work it should have shaped. Write review findings to a file as well, never only into a message.
- **Write only your own files.** Never write into another agent's unit file while it may still be running, or the late write clobbers one side.
- **Send a report only after reading its results.** A SendMessage issued in the same tool batch as the work it describes is written before that work's results exist.
- **Back every claim with a tool result.** Cite `path:line` for code and the `ls -la` line for a file, and quote the error when a call fails. The lead verifies each claim against git and disk before acting.
- **The lead owns every `bd` write.** Agents read beads but never create, update or close them, because concurrent embedded-Dolt writes contend for the lock.
