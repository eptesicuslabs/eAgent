import { createRoute } from "@tanstack/react-router";
import { useEffect } from "react";
import { ChatView } from "~/components/ChatView";
import { useStore } from "~/store";
import { Route as ChatRoute } from "./_chat";

export const Route = createRoute({
  getParentRoute: () => ChatRoute,
  path: "/threads/$threadId",
  component: ChatThreadRoute,
});

function ChatThreadRoute() {
  const { threadId } = Route.useParams();
  const setSelectedThreadId = useStore((state) => state.setSelectedThreadId);

  useEffect(() => {
    setSelectedThreadId(threadId);
  }, [setSelectedThreadId, threadId]);

  return <ChatView threadId={threadId} />;
}
