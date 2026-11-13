import { createFileRoute } from "@tanstack/react-router"

export const Route = createFileRoute("/binder-image/$card")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const { readCardImage } = await import("@/lib/binder.server")

        return readCardImage(params)
      },
    },
  },
})
