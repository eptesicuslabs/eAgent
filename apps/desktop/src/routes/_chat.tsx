import { Outlet, createRoute } from "@tanstack/react-router";
import { Sidebar } from "~/components/Sidebar";
import { Route as RootRoute } from "./__root";

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  id: "_chat",
  component: ChatLayoutRoute,
});

function ChatLayoutRoute() {
  return (
    <div className="grid h-screen w-full grid-cols-[280px_minmax(0,1fr)] overflow-hidden bg-background text-foreground">
      <Sidebar />
      <main className="min-w-0 overflow-hidden">
        <Outlet />
      </main>
    </div>
  );
}
