import {
  action,
  createAsync,
  useAction,
  useParams,
  useSubmission,
} from "@solidjs/router";
import { ErrorBoundary, For, Show, createSignal } from "solid-js";

const createUserAction = action(async (db: string, username: string) => {
  const resp = await fetch(`/api/database/${encodeURIComponent(db)}/user`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      username,
    }),
  });
  if (resp.status === 400) {
    throw new Error("Invalid credentials!");
  } else if (resp.status !== 200) {
    throw new Error(`Error ${resp.status} authenticating!`);
  }
  const data = await resp.json(); // Make sure the handler is done on the backend
  alert("User created with password: " + data.password);
});

const deleteUserAction = action(async (db: string, username: string) => {
  const resp = await fetch(
    `/api/database/${encodeURIComponent(db)}/user/${encodeURIComponent(
      username
    )}`,
    {
      method: "DELETE",
    }
  );
  if (resp.status === 400) {
    throw new Error("Invalid credentials!");
  } else if (resp.status !== 200) {
    throw new Error(`Error ${resp.status} authenticating!`);
  }
  await resp.text(); // Make sure the handler is done on the backend
});

const executeAction = action(async (db: string, query: string) => {
  const resp = await fetch(`/api/database/${encodeURIComponent(db)}/execute`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify(query),
  });
  if (resp.status === 400) {
    throw new Error("Invalid credentials!");
  } else if (resp.status !== 200) {
    throw new Error(`Error ${resp.status} authenticating!`);
  }
  return await resp.json();
});

export default function Page() {
  const params = useParams();
  const [refetch, setRefetch] = createSignal(0);
  const db = createAsync(async () => {
    refetch();
    const resp = await fetch(`/api/database/${params.dbId}`);
    if (!resp.ok) throw new Error(`Error fetching DBs ${resp.status}`);
    return await resp.json();
  });
  const createUserForm = useSubmission(createUserAction);
  const doCreateUser = useAction(createUserAction);
  const deleteUserForm = useSubmission(deleteUserAction);
  const doDeleteUser = useAction(deleteUserAction);
  const executeForm = useSubmission(executeAction);
  const doExecute = useAction(executeAction);
  const [query, setQuery] = createSignal("");
  const [result, setResult] = createSignal(null);

  return (
    <div class="p-4">
      <ErrorBoundary
        fallback={(error) => (
          <div class="text-red-500">Error: {error.message}</div>
        )}
      >
        <h1 class="font-bold text-4xl pb-4">{db()?.name}</h1>

        <h1 class="font-bold text-xl">Schema</h1>
        <div class="flex flex-col space-y-4">
          <For each={db()?.tables} fallback={<p>No tables found!</p>}>
            {(table) => (
              <details>
                <summary>{table.name}</summary>
                <pre>{table.schema}</pre>
              </details>
            )}
          </For>
        </div>

        <div class="flex justify-between">
          <h1 class="font-bold text-xl">Users</h1>

          <button
            class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
            onClick={() => {
              let username = prompt("Enter user's name:");
              if (!username) return;

              doCreateUser(params.dbId, username).then(() => {
                // TODO: Do this in the action so it's blocking the pending status
                setRefetch((v) => v + 1);
              });
            }}
            disabled={createUserForm.pending}
          >
            Create User
          </button>
        </div>
        <For each={db()?.users} fallback={<p>No users found!</p>}>
          {(user) => (
            <div class="flex justify-between border p-2">
              <p>{user.username}</p>

              <button
                class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
                onClick={() => {
                  doDeleteUser(params.dbId, user.username).then(() => {
                    // TODO: Do this in the action so it's blocking the pending status
                    setRefetch((v) => v + 1);
                  });
                }}
                disabled={deleteUserForm.pending}
              >
                Delete
              </button>
            </div>
          )}
        </For>

        <h1 class="font-bold text-xl">Connect</h1>
        {/* TODO: Show proper URL for Railway users + env var for others to set it properly */}
        <h3>Using DatabaseJS:</h3>
        <pre>https://username;{params.dbId}:password@localhost:2489</pre>
        {/* <h3>Using MySQL:</h3>
        <p>mysql://username:password@localhost:2489/{params.dbId}</p> */}

        <h1 class="font-bold text-xl">Execute</h1>
        <Show
          when={result}
          fallback={
            <Show when={executeForm.result}>
              {(result) => (
                <pre class="border p-4">
                  {JSON.stringify(result(), null, 2)}
                </pre>
              )}
            </Show>
          }
        >
          {(result) => <pre class="border p-4">{result()}</pre>}
        </Show>

        <textarea
          class="border"
          value={query()}
          onChange={(e) => setQuery(e.target.value)}
          disabled={executeForm.pending}
          placeholder="SELECT 1;"
        ></textarea>
        <button
          disabled={executeForm.pending || query() === ""}
          onClick={(e) => {
            e.preventDefault();
            const sql = query();
            Promise.all(
              sql
                .split("--> statement-breakpoint")
                .map((stmt) => doExecute(params.dbId, stmt))
            ).then((results) => {
              setResult(
                results
                  .map((result) => JSON.stringify(result, null, 2))
                  .join("\n\n")
              );
            });
          }}
        >
          Run
        </button>
      </ErrorBoundary>
    </div>
  );
}
