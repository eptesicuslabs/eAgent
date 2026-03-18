import { Outlet, createRoute } from "@tanstack/react-router";
import { Sidebar } from "~/components/Sidebar";
import { TopBar } from "~/components/TopBar";
import { Route as RootRoute } from "./__root";
import { useNavigate } from "@tanstack/react-router";

export const Route = createRoute({
  getParentRoute: () => RootRoute,
  id: "_chat",
  component: ChatLayoutRoute,
});

function ChatLayoutRoute() {
  const navigate = useNavigate();

  return (
    <div className="grid h-screen w-full grid-rows-[auto_minmax(0,1fr)] overflow-hidden bg-background text-foreground">
      {/* Top bar spans full width */}
      <TopBar
        onSettingsClick={() => {
          void navigate({ to: "/settings" });
        }}
      />

      {/* Main area: sidebar + content */}
      <div className="grid min-h-0 grid-cols-[280px_minmax(0,1fr)] overflow-hidden">
        <Sidebar />
        <main className="min-w-0 overflow-hidden">
          <Outlet />
        </main>
      </div>
    </div>
  );
}
