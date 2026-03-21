import { Outlet, createRoute } from "@tanstack/react-router";
import { Sidebar } from "~/components/Sidebar";
import { TopBar } from "~/components/TopBar";
import { Route as RootRoute } from "./__root";

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  id: "_chat",
  component: ChatLayoutRoute,
});

function ChatLayoutRoute() {
  return (
    <div className="grid h-screen w-full grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-background text-foreground">
      <TopBar />
      <div className="grid min-h-0 grid-cols-[200px_minmax(0,1fr)] overflow-hidden">
        <Sidebar />
        <main className="min-w-0 overflow-hidden">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
