import { RouteDefinition } from "@solidjs/router";
import { lazy } from "solid-js";

export default [
  {
    path: "/login",
    component: lazy(() => import("./login.tsx")),
  },
  {
    component: lazy(() => import("./layout.tsx")),
    children: [
      {
        path: "/",
        component: lazy(() => import("./index.tsx")),
      },
      {
        path: "/:dbId",
        component: lazy(() => import("./[dbId].tsx")),
      },
      {
        path: "/settings",
        component: lazy(() => import("./settings/index.tsx")),
      },
      {
        path: "/backups",
        component: lazy(() => import("./backups.tsx")),
      },
    ],
  },
] satisfies RouteDefinition[];
