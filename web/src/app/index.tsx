import {
  A,
  action,
  createAsync,
  redirect,
  useAction,
  useSubmission,
} from "@solidjs/router";
import { For, createSignal } from "solid-js";

const createDbAction = action(async (name: string) => {
  const resp = await fetch("/api/database", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      name,
    }),
  });
  if (resp.status === 400) {
    throw new Error("Invalid credentials!");
  } else if (resp.status !== 200) {
    throw new Error(`Error ${resp.status} authenticating!`);
  }
  await resp.text(); // Make sure the handler is done on the backend

  throw redirect(`/${name}`);
});

const deleteDBAction = action(async (name: string) => {
  const resp = await fetch(`/api/database/${encodeURIComponent(name)}`, {
    method: "DELETE",
    headers: {
      "Content-Type": "application/json",
    },
  });
  if (resp.status === 400) {
    throw new Error("Invalid credentials!");
  } else if (resp.status !== 200) {
    throw new Error(`Error ${resp.status} authenticating!`);
  }
  await resp.text(); // Make sure the handler is done on the backend
});

export default function Page() {
  const [refetch, setRefetch] = createSignal(0);
  const dbs = createAsync(async () => {
    refetch();
    const resp = await fetch("/api/database");
    if (!resp.ok) throw new Error(`Error fetching DBs ${resp.status}`);
    return await resp.json();
  });
  const createForm = useSubmission(createDbAction);
  const doCreateDB = useAction(createDbAction);
  const deleteForm = useSubmission(deleteDBAction);
  const doDeleteDB = useAction(deleteDBAction);

  return (
    <div class="p-4">
      <div class="flex justify-between">
        <h1 class="font-bold text-2xl">Databases:</h1>
        <button
          class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
          onClick={() => {
            let name = prompt("Enter database name:");
            if (!name) return;

            doCreateDB(name);
          }}
          disabled={createForm.pending}
        >
          Create DB
        </button>
      </div>
      <ul class="flex flex-col space-y-4">
        <For each={dbs()} fallback={<li>No databases found!</li>}>
          {(db) => (
            <li class="border p-4 flex justify-between">
              <A href={`/${db.name}`}>{db.name}</A>
              <button
                class="bg-red-500 hover:bg-red-700 text-white font-bold py-2 px-4 rounded"
                onClick={() => {
                  doDeleteDB(db.name).then(() => {
                    // TODO: Do this in the action so it's blocking the pending status
                    setRefetch((v) => v + 1);
                  });
                }}
                disabled={deleteForm.pending}
              >
                Delete
              </button>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
}
