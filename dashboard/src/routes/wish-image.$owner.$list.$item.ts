import { createFileRoute } from "@tanstack/react-router"

export const Route = createFileRoute("/wish-image/$owner/$list/$item")({
  server: {
    handlers: {
      GET: async ({ params }) => {
        const { readWishImage } = await import("@/lib/wishlist.server")

        return readWishImage(params)
      },
    },
  },
})
