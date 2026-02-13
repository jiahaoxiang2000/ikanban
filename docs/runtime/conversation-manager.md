# ConversationManager

`src/runtime/conversation-manager.ts` is a runtime-facing adapter that wraps OpenCode client APIs for sessions, prompts, messages, and events.

It does four main things:

- validates and normalizes all user/runtime input before calling the SDK
- tracks in-memory relationships between task IDs, session IDs, and worktree directories
- provides awaitable prompt APIs that return after assistant output is observed
- returns SDK-native message payloads and tracks state transitions safely

## Core State

`ConversationManager` keeps three in-memory maps:

- `taskToSessionID`: maps a task to its conversation session
- `sessionToDirectory`: maps a session to the worktree directory where requests should run
- `sessionsByID`: caches normalized session metadata (`ConversationSessionMeta`)

These maps are updated when a session is created and when prompts are submitted.

## Session Creation Flow

`createTaskSession(input)`:

1. normalizes `projectId`, `taskId`, directories, optional `title`, and timestamp
2. gets a runtime client scoped to the worktree directory
3. calls `client.session.create(...)`
4. normalizes response fields (`sessionID`, timestamps)
5. stores session/task/directory mappings in memory
6. returns a `ConversationSessionMeta`

If the SDK response has an error or missing data, it throws with a descriptive message.

## Prompt APIs

Prompt submission is awaitable only:

- `sendInitialPromptAndAwaitMessages(...)` / `sendFollowUpPromptAndAwaitMessages(...)`
  - uses `client.session.promptAsync(...)`
  - subscribes to runtime events
  - polls messages and returns only when the session becomes idle (or timeout/error)
  - returns `PromptExecutionResult` with:
    - `submission` metadata
    - `sdkMessages` (`SessionMessagesResponses[200][number][]`)

Model selection is resolved via `resolvePromptModel(...)` when no explicit model is provided.

## `sendPromptAndAwaitMessages(...)` Detailed Logic

`sendInitialPromptAndAwaitMessages(...)` and `sendFollowUpPromptAndAwaitMessages(...)` both call the same private method: `sendPromptAndAwaitMessages(...)`.

This method is the core execution path and runs in this order:

1. **Normalize input and resolve runtime context**
   - normalize `sessionID`, `prompt`, and `timeoutMs`
   - resolve `worktreeDirectory` from explicit input or session mapping
   - get scoped client from runtime
   - resolve optional model/agent

2. **Build message baseline**
   - fetch current session messages before sending prompt
   - store each message state signature in `knownMessageStates`
   - this baseline prevents old messages from being returned as "new"

3. **Prepare incremental collector**
   - create `pollForNewMessages()`
   - each poll calls `collectMessageChanges(...)` and appends only changed/new messages
   - invoke `onMessage` callback for each newly observed message

4. **Subscribe and submit**
   - subscribe to event stream with `client.event.subscribe(...)`
   - submit prompt via `client.session.promptAsync(...)`
   - if prompt submission fails, log structured error and throw immediately
   - update cached `sessionsByID` timestamps on successful submission

5. **Wait loop with event + polling coordination**
   - call `waitForSessionIdle(...)` using the event iterator
   - on each session event, if it is a message stream event (`message.updated`, `message.part.updated`, etc.), force poll immediately
   - on periodic ticks (timeout between events), poll messages
   - stop condition: root session reaches idle/completed status
   - propagate root `session.error` as failure

6. **Finalize and enforce response guarantees**
   - force one final poll after loop and once after stream close path
   - require at least one assistant-role message in collected results
   - if none is found, throw timeout/no-response error
   - return `PromptExecutionResult` with submission metadata and `sdkMessages`

Why this design is used:

- Event stream gives fast reaction to updates.
- Polling guarantees message snapshots are complete and normalized.
- Signature comparison avoids duplicate message emissions.
- Final assistant-message check ensures caller gets a real model response, not only status transitions.

## How "Await Messages" Works

The await flow combines event streaming with message polling:

1. list current messages and record their signatures (id + state)
2. subscribe to `client.event.subscribe(...)`
3. submit prompt asynchronously
4. wait for session activity to settle using `waitForSessionIdle(...)`
5. on message stream events (`message.updated`, `message.part.updated`, etc.), force a message poll
6. compare message signatures and collect only changed/new messages
7. stop when:
   - root session is idle/completed

If no assistant message is observed by timeout, it throws.

## Event Handling

`waitForSessionIdle(...)` waits only on the target root session.

- processes events for the requested `sessionID`
- ignores child-session lifecycle for stop conditions
- returns early with an error message when a root `session.error` event is received

## `waitForSessionIdle(...)` Detailed Logic

`waitForSessionIdle(...)` is the gate that decides when the async prompt flow can stop waiting.

Execution model:

1. set an absolute timeout window (`deadline = now + timeoutMs`)
2. read next event from async iterator with a bounded wait (`nextEventWithTimeout`, max 1s per poll)
3. if no event arrives in that slice, call `onTick()` hook and continue
4. normalize incoming event payload (`normalizeEvent`) and optionally forward it to `onEvent()` hook
5. ignore events that do not belong to the target `sessionID` (`extractEventSessionID`)
6. for matching session events:
   - call `onSessionEvent()` hook
   - extend timeout window (`nextDeadline = now + timeoutMs`) to allow active runs to continue
7. stop with failure if event is `session.error` (extracts best error message from payload)
8. stop with success if root session becomes idle/completed (`session.idle`, `session.completed`, or status `idle/completed/done`) after activity has been seen
9. if stream ends or timeout elapses without success, return `{ idle: false }`

Key behavior details:

- **Activity guard**: it does not treat an immediate idle status as completion until it has seen some non-idle session activity (`sawSessionActivity`).
- **Sliding timeout**: each relevant session event refreshes `nextDeadline`, so long-running active sessions do not fail just because the initial timeout window was short.
- **Root-only stop**: only the requested root `sessionID` controls completion; child session state does not block return.

## SDK Event Types

From `node_modules/@opencode-ai/sdk/dist/v2/gen/types.gen.d.ts` (generated from the SDK `types.gen.ts`), the `EventXX` payload types are:

- `EventInstallationUpdated`
- `EventInstallationUpdateAvailable`
- `EventProjectUpdated`
- `EventServerInstanceDisposed`
- `EventServerConnected`
- `EventGlobalDisposed`
- `EventLspClientDiagnostics`
- `EventLspUpdated`
- `EventFileEdited`
- `EventMessageUpdated`
- `EventMessageRemoved`
- `EventMessagePartUpdated`
- `EventMessagePartRemoved`
- `EventPermissionAsked`
- `EventPermissionReplied`
- `EventSessionStatus`
- `EventSessionIdle`
- `EventQuestionAsked`
- `EventQuestionReplied`
- `EventQuestionRejected`
- `EventSessionCompacted`
- `EventFileWatcherUpdated`
- `EventTodoUpdated`
- `EventTuiPromptAppend`
- `EventTuiCommandExecute`
- `EventTuiToastShow`
- `EventTuiSessionSelect`
- `EventMcpToolsChanged`
- `EventMcpBrowserOpenFailed`
- `EventCommandExecuted`
- `EventSessionCreated`
- `EventSessionUpdated`
- `EventSessionDeleted`
- `EventSessionDiff`
- `EventSessionError`
- `EventVcsBranchUpdated`
- `EventPtyCreated`
- `EventPtyUpdated`
- `EventPtyExited`
- `EventPtyDeleted`
- `EventWorktreeReady`
- `EventWorktreeFailed`

## Message Change Detection

Messages are tracked in SDK-native shape, and updates are detected using `toMessageStateSignature(...)`.

- signature includes role, created time, text preview from text parts, part count, and error presence
- previously seen signatures are stored by message id in `knownMessageStates`
- when signature changes, the message is emitted as a newly observed update

## Input/Output Guardrails

The file includes many small normalizers and wrappers:

- id/session/prompt/title/directory/timestamp/timeout validation
- safe conversion from unknown values (`asRecord`)
- unified SDK error formatting (`formatUnknownError`)
- response unwrapping helpers (`readDataOrThrow`, `unwrapResponseDataOrThrow`)

These helpers keep API-facing methods small and make failures explicit.

## Public API Summary

- `createTaskSession(...)`
- `sendInitialPromptAndAwaitMessages(...)`
- `sendFollowUpPromptAndAwaitMessages(...)`
- `listConversationMessages(...)`
- `subscribeToEvents(...)`
- `getTaskSessionID(...)`
- `getSessionDirectory(...)`
- `getSession(...)`

In short, `ConversationManager` is the orchestration layer between UI/task logic and OpenCode session/event APIs, with strict normalization and robust wait-until-idle behavior.
