import { createMiddleware, createStart } from "@tanstack/react-start"
import { dataModeCookie, dataModeSwitch } from "./lib/demo/data-mode"

const chooseDataMode = createMiddleware({ type: "request" }).server(
  ({ request, next }) => {
    const choice =
      request.method === "GET"
        ? dataModeSwitch(new URL(request.url))
        : undefined

    if (choice === undefined) return next()

    return new Response(null, {
      status: 303,
      headers: {
        Location: choice.location,
        "Set-Cookie": dataModeCookie(choice.mode),
        "Cache-Control": "no-store",
      },
    })
  }
)

export const startInstance = createStart(() => ({
  requestMiddleware: [chooseDataMode],
}))
