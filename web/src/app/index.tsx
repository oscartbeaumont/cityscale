import { createAsync } from "@solidjs/router";
import { Suspense } from "solid-js";

export default function Page() {
  const version = createAsync(async () => {
    // TODO: Proper HTTP error handling
    return await (await fetch("/api/version")).text();
  });
  return (
    <>
      <h1 class="bg-red-500">Hello World</h1>
      <Suspense fallback={<p>Loading...</p>}>
        <p>{version()}</p>
      </Suspense>
    </>
  );
}
