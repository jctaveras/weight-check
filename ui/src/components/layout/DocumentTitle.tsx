import { useRouterState } from "@tanstack/react-router";
import { useEffect } from "react";

const APP_TITLE = "weight-check";

function documentTitle(title: string | undefined) {
  return title ? `${title} · ${APP_TITLE}` : APP_TITLE;
}

/** Sets `document.title` from the deepest matched route's `staticData.title`. */
export function DocumentTitle() {
  const title = useRouterState({
    select: (state) => state.matches.findLast((match) => match.staticData.title)?.staticData.title,
  });
  useEffect(() => {
    document.title = documentTitle(title);
  }, [title]);
  return null;
}
