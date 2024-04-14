import { action, redirect, useSubmission } from "@solidjs/router";
import { Show } from "solid-js";

const loginAction = action(async (data: FormData) => {
  const resp = await fetch("/api/login", {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
    body: JSON.stringify({
      username: data.get("username"),
      password: data.get("password"),
    }),
  });
  if (resp.status === 400) {
    throw new Error("Invalid credentials!");
  } else if (resp.status !== 200) {
    throw new Error(`Error ${resp.status} authenticating!`);
  }

  throw redirect("/");
});

export default function Page() {
  const form = useSubmission(loginAction);

  return (
    <div>
      <h1 class="font-bold text-2xl">Cityscale</h1>
      <form action={loginAction} method="post" class="size-1/3">
        <Show when={form.error}>
          {(error) => <p class="text-red-500">{error().toString()}</p>}
        </Show>
        <fieldset disabled={form.pending} class="flex flex-col">
          <label>
            Email:
            <input
              name="username"
              autocomplete="username"
              placeholder="admin"
            />
          </label>
          <label>
            Password:
            <input
              name="password"
              type="password"
              autocomplete="current-password"
              placeholder="password"
            />
          </label>
          <button type="submit">Login</button>
        </fieldset>
      </form>
    </div>
  );
}
