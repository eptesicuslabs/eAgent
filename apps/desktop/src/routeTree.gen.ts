import { Route as rootRoute } from "./routes/__root";
import { Route as chatRoute } from "./routes/_chat";
import { Route as chatIndexRoute } from "./routes/_chat.index";
import { Route as chatThreadRoute } from "./routes/_chat.$threadId";
import { Route as chatSettingsRoute } from "./routes/_chat.settings";

const chatRouteTree = chatRoute.addChildren([
  chatIndexRoute,
  chatThreadRoute,
  chatSettingsRoute,
]);

export const routeTree = rootRoute.addChildren([chatRouteTree]);
