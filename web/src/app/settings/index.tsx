import { action, createAsync, useAction, useSubmission } from "@solidjs/router";
import { For, Show, Suspense, createSignal } from "solid-js";

const createAdminAction = action(async (username: string, password: string) => {
  const resp = await fetch("/api/settings/admin", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      username,
      password,
    }),
  });
  if (resp.status === 400) {
    throw new Error("Invalid credentials!");
  } else if (resp.status !== 201) {
    throw new Error(`Error ${resp.status} creating user!`);
  }
  await resp.text(); // Make sure the handler is done on the backend
});

const deleteAdminAction = action(async (username: string) => {
  const resp = await fetch(
    `/api/settings/admin/${encodeURIComponent(username)}`,
    {
      method: "DELETE",
    }
  );
  if (resp.status === 400) {
    throw new Error("Invalid credentials!");
  } else if (resp.status !== 204) {
    throw new Error(`Error ${resp.status} deleting user!`);
  }
  await resp.text(); // Make sure the handler is done on the backend
});

export default function Page() {
  const [refetch, setRefetch] = createSignal(0);
  const version = createAsync(async () => {
    const resp = await fetch(`/api/version`);
    if (!resp.ok) throw new Error(`Error fetching version ${resp.status}`);
    return await resp.text();
  });
  const admins = createAsync(async () => {
    // TODO: Bruh this is so hacky
    if (refetch() !== 0) await new Promise((r) => setTimeout(r, 500));

    const resp = await fetch(`/api/settings/admin`);
    if (!resp.ok) throw new Error(`Error fetching admins ${resp.status}`);
    return await resp.json();
  });
  const createForm = useSubmission(createAdminAction);
  const doCreateAdmin = useAction(createAdminAction);
  const deleteForm = useSubmission(deleteAdminAction);
  const doDeleteAdmin = useAction(deleteAdminAction);

  return (
    <div>
      <h1 class="font-bold text-4xl pb-4">Settings</h1>

      <p>
        Version: <Suspense fallback="...">{version()}</Suspense>
      </p>

      <Suspense>
        <div class="flex justify-between">
          <h2 class="font-bold text-2xl pt-4 pb-2">Admins</h2>

          <button
            class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
            onClick={() => {
              const username = prompt("Enter the username");
              if (!username) return;
              const password = prompt("Enter the password");
              if (!password) return;

              doCreateAdmin(username, password).then(() => {
                // TODO: Do this in the action so it's blocking the pending status
                setRefetch((v) => v + 1);
              });
            }}
            disabled={createForm.pending}
          >
            Create Admin
          </button>
        </div>
        <ul class="p-4 flex flex-col space-y-4 justify-between">
          <For each={admins()}>
            {(admin) => (
              <li class="border p-4 flex justify-between w-full">
                <p>{admin.username}</p>

                <div class="flex space-x-4">
                  <button
                    class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
                    onClick={() => {
                      const password = prompt("Enter the new password");
                      if (!password) return;

                      doCreateAdmin(admin.username, password).then(() => {
                        // TODO: Do this in the action so it's blocking the pending status
                        setRefetch((v) => v + 1);
                      });
                    }}
                    disabled={createForm.pending}
                  >
                    Edit password
                  </button>

                  <Show when={!admin.is_self}>
                    <button
                      class="bg-red-500 hover:bg-red-700 text-white font-bold py-2 px-4 rounded"
                      onClick={() => {
                        doDeleteAdmin(admin.username).then(() => {
                          // TODO: Do this in the action so it's blocking the pending status
                          setRefetch((v) => v + 1);
                        });
                      }}
                      disabled={deleteForm.pending}
                    >
                      Delete
                    </button>
                  </Show>
                </div>
              </li>
            )}
          </For>
        </ul>
      </Suspense>
    </div>
  );
}
