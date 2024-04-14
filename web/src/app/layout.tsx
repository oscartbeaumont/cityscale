import {
  A,
  action,
  createAsync,
  redirect,
  useAction,
  useNavigate,
  useSubmission,
} from "@solidjs/router";
import { ErrorBoundary, ParentProps, Suspense } from "solid-js";
import logo from "../assets/logo.png";

const logoutAction = action(async () => {
  const resp = await fetch("/api/logout", {
    method: "POST",
  });
  if (resp.status !== 200) {
    throw new Error(`Error ${resp.status} logging out!`);
  }
  throw redirect("/login");
});

export default function Page(props: ParentProps) {
  const navigate = useNavigate();
  const auth = createAsync(async () => {
    const resp = await fetch("/api/me");
    if (resp.status === 401) {
      navigate("/login");
    } else if (resp.status !== 200) {
      throw new Error(`Error ${resp.status} authenticating!`);
    }

    return await resp.text();
  });
  const logout = useAction(logoutAction);
  const logoutSubmission = useSubmission(logoutAction);

  return (
    <>
      <ErrorBoundary
        fallback={(err) => <p class="text-red-500">{err.toString()}</p>}
      >
        <Suspense fallback={<p>Authenticating...</p>}>
          <div class="flex space-x-4 items-center p-4">
            <A href="/">
              <img
                src={logo}
                class="w-12 h-12 hover:scale-105"
                alt="Cityscale Logo"
              />
            </A>
            <p>Authenticated as: {auth()}</p>
            <button
              onClick={() => logout()}
              disabled={logoutSubmission.pending}
            >
              Logout
            </button>
          </div>
          <ErrorBoundary
            fallback={(err) => <p class="text-red-500">{err.toString()}</p>}
          >
            <Suspense fallback={<p>Loading...</p>}>{props.children}</Suspense>
          </ErrorBoundary>
        </Suspense>
      </ErrorBoundary>
    </>
  );
}
